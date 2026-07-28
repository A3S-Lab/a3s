use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use a3s_updater::InstallProvenance;
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use super::discovery::find_state;
use super::id::ComponentId;
use super::lifecycle::OperationRecord;
use super::paths::ComponentPaths;
use super::state::{Health, Presence};

const JOURNAL_SCHEMA_VERSION: u32 = 1;
const MAX_JOURNAL_BYTES: u64 = 1024 * 1024;
const RECOVERED_MESSAGE_PREFIX: &str = "Recovered completed checkpoint: ";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum BatchPhase {
    Applying,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum StepStatus {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JournalOperation {
    component: ComponentId,
    action: String,
    changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provenance: Option<InstallProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
    message: String,
}

impl JournalOperation {
    fn from_operation(operation: &OperationRecord) -> Self {
        Self {
            component: operation.component.clone(),
            action: operation.action.to_string(),
            changed: operation.changed,
            version: operation.version.clone(),
            provenance: operation.provenance,
            path: operation.path.clone(),
            message: operation.message.clone(),
        }
    }

    fn recovered(&self, action: &'static str) -> anyhow::Result<OperationRecord> {
        if self.action != action {
            bail!(
                "component journal action '{}' does not match recovery action '{}'",
                self.action,
                action
            );
        }
        Ok(OperationRecord {
            component: self.component.clone(),
            action,
            changed: self.changed,
            recovered: true,
            version: self.version.clone(),
            provenance: self.provenance,
            path: self.path.clone(),
            message: if self.message.starts_with(RECOVERED_MESSAGE_PREFIX) {
                self.message.clone()
            } else {
                format!("{RECOVERED_MESSAGE_PREFIX}{}", self.message)
            },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JournalStep {
    component: ComponentId,
    status: StepStatus,
    #[serde(default, skip_serializing_if = "is_false")]
    recovered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operation: Option<JournalOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    failure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchRecord {
    schema_version: u32,
    command: String,
    action: String,
    plan_digest: String,
    phase: BatchPhase,
    owner_pid: u32,
    started_at: String,
    updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recovered_from_plan_digest: Option<String>,
    steps: Vec<JournalStep>,
}

pub(super) struct BatchJournal {
    active_path: PathBuf,
    last_path: PathBuf,
    record: BatchRecord,
    recovered: BTreeMap<ComponentId, OperationRecord>,
}

impl BatchJournal {
    pub(super) fn begin(
        paths: &ComponentPaths,
        command: &'static str,
        action: &'static str,
        plan_digest: &str,
        components: &[ComponentId],
    ) -> anyhow::Result<Self> {
        validate_digest(plan_digest)?;
        validate_batch_components(components)?;
        let active_path = paths.active_operation_journal_path();
        let last_path = paths.last_operation_journal_path();
        let interrupted_path = paths.interrupted_operation_journal_path();
        let previous = read_record(&active_path)?;
        let mut recovered = BTreeMap::new();
        let mut recovered_from_plan_digest = None;

        if let Some(mut previous) = previous {
            if previous.phase == BatchPhase::Applying {
                let same_batch = previous.command == command
                    && previous.action == action
                    && previous
                        .steps
                        .iter()
                        .map(|step| &step.component)
                        .eq(components.iter());
                if same_batch {
                    recovered_from_plan_digest = Some(previous.plan_digest.clone());
                    for step in &previous.steps {
                        let Some(operation) = step
                            .operation
                            .as_ref()
                            .filter(|_| step.status == StepStatus::Succeeded)
                        else {
                            continue;
                        };
                        if operation_still_applied(operation, action, paths) {
                            recovered.insert(step.component.clone(), operation.recovered(action)?);
                        }
                    }
                }
                previous.phase = BatchPhase::Interrupted;
                previous.updated_at = now();
                write_record(&interrupted_path, &previous)?;
            } else {
                write_record(&last_path, &previous)?;
            }
        }

        let timestamp = now();
        let steps = components
            .iter()
            .map(|component| match recovered.get(component) {
                Some(operation) => JournalStep {
                    component: component.clone(),
                    status: StepStatus::Succeeded,
                    recovered: true,
                    operation: Some(JournalOperation::from_operation(operation)),
                    failure: None,
                },
                None => JournalStep {
                    component: component.clone(),
                    status: StepStatus::Pending,
                    recovered: false,
                    operation: None,
                    failure: None,
                },
            })
            .collect();
        let record = BatchRecord {
            schema_version: JOURNAL_SCHEMA_VERSION,
            command: command.to_string(),
            action: action.to_string(),
            plan_digest: plan_digest.to_string(),
            phase: BatchPhase::Applying,
            owner_pid: std::process::id(),
            started_at: timestamp.clone(),
            updated_at: timestamp,
            recovered_from_plan_digest,
            steps,
        };
        write_record(&active_path, &record)?;
        Ok(Self {
            active_path,
            last_path,
            record,
            recovered,
        })
    }

    pub(super) fn take_recovered(&mut self, component: &ComponentId) -> Option<OperationRecord> {
        self.recovered.remove(component)
    }

    pub(super) fn record_success(&mut self, operation: &OperationRecord) -> anyhow::Result<()> {
        let step = self.pending_step_mut(&operation.component)?;
        step.status = StepStatus::Succeeded;
        step.operation = Some(JournalOperation::from_operation(operation));
        step.failure = None;
        self.checkpoint()
    }

    pub(super) fn record_failure(
        &mut self,
        component: &ComponentId,
        failure: &str,
    ) -> anyhow::Result<()> {
        let step = self.pending_step_mut(component)?;
        step.status = StepStatus::Failed;
        step.operation = None;
        step.failure = Some(failure.to_string());
        self.checkpoint()
    }

    pub(super) fn finish(mut self, success: bool) -> anyhow::Result<()> {
        if self
            .record
            .steps
            .iter()
            .any(|step| step.status == StepStatus::Pending)
        {
            bail!("component batch journal cannot finish with pending steps");
        }
        let has_failures = self
            .record
            .steps
            .iter()
            .any(|step| step.status == StepStatus::Failed);
        if success == has_failures {
            bail!("component batch journal result does not match its step outcomes");
        }
        self.record.phase = if success {
            BatchPhase::Completed
        } else {
            BatchPhase::Failed
        };
        self.record.updated_at = now();
        write_record(&self.active_path, &self.record)?;
        write_record(&self.last_path, &self.record)?;
        remove_if_present(&self.active_path)?;
        sync_parent(&self.active_path)?;
        Ok(())
    }

    fn pending_step_mut(&mut self, component: &ComponentId) -> anyhow::Result<&mut JournalStep> {
        let step = self
            .record
            .steps
            .iter_mut()
            .find(|step| &step.component == component)
            .with_context(|| {
                format!("component '{}' is absent from the batch journal", component)
            })?;
        if step.status != StepStatus::Pending {
            bail!(
                "component '{}' journal step is already {:?}",
                component,
                step.status
            );
        }
        Ok(step)
    }

    fn checkpoint(&mut self) -> anyhow::Result<()> {
        self.record.updated_at = now();
        write_record(&self.active_path, &self.record)
    }
}

fn operation_still_applied(
    operation: &JournalOperation,
    action: &str,
    paths: &ComponentPaths,
) -> bool {
    let Ok(state) = find_state(&operation.component, paths) else {
        return false;
    };
    if action == "uninstall" {
        return state.presence != Presence::Managed;
    }
    if !state.is_ready() || state.health != Health::Ready {
        return false;
    }
    if operation
        .version
        .as_deref()
        .is_some_and(|version| state.version.as_deref() != Some(version))
    {
        return false;
    }
    if operation
        .provenance
        .is_some_and(|provenance| state.provenance != Some(provenance))
    {
        return false;
    }
    if operation
        .path
        .as_deref()
        .is_some_and(|path| state.path.as_deref() != Some(path))
    {
        return false;
    }
    true
}

pub(super) fn validate_batch_components(components: &[ComponentId]) -> anyhow::Result<()> {
    if components.is_empty() {
        bail!("component batch journal requires at least one component");
    }
    let mut unique = BTreeSet::new();
    for component in components {
        ComponentId::parse(component.to_string())?;
        if !unique.insert(component) {
            bail!(
                "component '{}' appears more than once in the batch",
                component
            );
        }
    }
    Ok(())
}

fn validate_record(record: &BatchRecord) -> anyhow::Result<()> {
    if record.schema_version != JOURNAL_SCHEMA_VERSION {
        bail!(
            "unsupported component journal schema {}",
            record.schema_version
        );
    }
    if !matches!(
        (record.command.as_str(), record.action.as_str()),
        ("component.install", "install")
            | ("component.upgrade", "upgrade")
            | ("component.uninstall", "uninstall")
    ) {
        bail!("component journal command/action pair is invalid");
    }
    validate_digest(&record.plan_digest)?;
    if let Some(digest) = &record.recovered_from_plan_digest {
        validate_digest(digest)?;
    }
    if record.started_at.is_empty() || record.updated_at.is_empty() {
        bail!("component journal timestamps are missing");
    }
    let components = record
        .steps
        .iter()
        .map(|step| step.component.clone())
        .collect::<Vec<_>>();
    validate_batch_components(&components)?;
    for step in &record.steps {
        match step.status {
            StepStatus::Pending if step.operation.is_none() && step.failure.is_none() => {}
            StepStatus::Succeeded if step.operation.is_some() && step.failure.is_none() => {
                let operation = step.operation.as_ref().expect("checked above");
                if operation.component != step.component || operation.action != record.action {
                    bail!("component journal success checkpoint is internally inconsistent");
                }
            }
            StepStatus::Failed if step.operation.is_none() && step.failure.is_some() => {}
            _ => bail!("component journal step payload does not match its status"),
        }
        if step.recovered && step.status != StepStatus::Succeeded {
            bail!("only a successful component journal step can be recovered");
        }
    }
    let has_pending = record
        .steps
        .iter()
        .any(|step| step.status == StepStatus::Pending);
    let has_failures = record
        .steps
        .iter()
        .any(|step| step.status == StepStatus::Failed);
    match record.phase {
        BatchPhase::Completed if has_pending || has_failures => {
            bail!("completed component journal has unfinished step outcomes")
        }
        BatchPhase::Failed if has_pending || !has_failures => {
            bail!("failed component journal has inconsistent step outcomes")
        }
        BatchPhase::Applying
        | BatchPhase::Interrupted
        | BatchPhase::Completed
        | BatchPhase::Failed => {}
    }
    Ok(())
}

fn validate_digest(value: &str) -> anyhow::Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        bail!("component journal plan digest is invalid");
    }
    Ok(())
}

fn read_record(path: &Path) -> anyhow::Result<Option<BatchRecord>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!(
            "component journal '{}' must be a regular file",
            path.display()
        );
    }
    if metadata.len() == 0 || metadata.len() > MAX_JOURNAL_BYTES {
        bail!("component journal '{}' has an invalid size", path.display());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .with_context(|| format!("failed to open component journal {}", path.display()))?
        .take(MAX_JOURNAL_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read component journal {}", path.display()))?;
    let record: BatchRecord = serde_json::from_slice(&bytes)
        .with_context(|| format!("component journal '{}' is invalid", path.display()))?;
    validate_record(&record)?;
    Ok(Some(record))
}

fn write_record(path: &Path, record: &BatchRecord) -> anyhow::Result<()> {
    validate_record(record)?;
    let parent = path
        .parent()
        .context("component journal path has no parent directory")?;
    ensure_real_directory(parent)?;
    let bytes = serde_json::to_vec_pretty(record).context("failed to encode component journal")?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        bail!("component journal exceeds its size limit");
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "failed to create temporary component journal in {}",
            parent.display()
        )
    })?;
    set_private_file(temporary.as_file())?;
    temporary
        .write_all(&bytes)
        .context("failed to write component journal")?;
    temporary
        .as_file()
        .sync_all()
        .context("failed to sync component journal")?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to publish component journal {}", path.display()))?;
    sync_parent(path)
}

fn ensure_real_directory(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("failed to create journal directory {}", path.display()))?;
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect journal directory {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "journal directory '{}' must be a real directory",
            path.display()
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn set_private_file(file: &File) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn sync_parent(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let parent = path.parent().context("journal path has no parent")?;
        File::open(parent)
            .with_context(|| format!("failed to open journal directory {}", parent.display()))?
            .sync_all()
            .with_context(|| format!("failed to sync journal directory {}", parent.display()))?;
    }
    Ok(())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn bundled_operation(paths: &ComponentPaths) -> OperationRecord {
        OperationRecord {
            component: ComponentId::parse("code").unwrap(),
            action: "install",
            changed: false,
            recovered: false,
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            provenance: Some(InstallProvenance::Bundled),
            path: Some(paths.current_exe.clone()),
            message: "Code is bundled with a3s.".to_string(),
        }
    }

    #[test]
    fn successful_journal_is_checkpointed_and_finalized() {
        let temp = tempfile::tempdir().unwrap();
        let paths = ComponentPaths::for_test(temp.path());
        let component = ComponentId::parse("code").unwrap();
        let mut journal = BatchJournal::begin(
            &paths,
            "component.install",
            "install",
            &digest('a'),
            std::slice::from_ref(&component),
        )
        .unwrap();
        assert!(paths.active_operation_journal_path().is_file());
        journal.record_success(&bundled_operation(&paths)).unwrap();
        journal.finish(true).unwrap();
        assert!(!paths.active_operation_journal_path().exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                std::fs::metadata(paths.operation_journal_root())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(paths.last_operation_journal_path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let last: serde_json::Value =
            serde_json::from_slice(&std::fs::read(paths.last_operation_journal_path()).unwrap())
                .unwrap();
        assert_eq!(last["phase"], "completed");
        assert_eq!(last["steps"][0]["status"], "succeeded");
    }

    #[test]
    fn unfinished_journal_recovers_only_a_still_valid_checkpoint() {
        let temp = tempfile::tempdir().unwrap();
        let paths = ComponentPaths::for_test(temp.path());
        let component = ComponentId::parse("code").unwrap();
        let mut first = BatchJournal::begin(
            &paths,
            "component.install",
            "install",
            &digest('a'),
            std::slice::from_ref(&component),
        )
        .unwrap();
        first.record_success(&bundled_operation(&paths)).unwrap();
        drop(first);

        let mut resumed = BatchJournal::begin(
            &paths,
            "component.install",
            "install",
            &digest('b'),
            std::slice::from_ref(&component),
        )
        .unwrap();
        let operation = resumed.take_recovered(&component).unwrap();
        assert!(operation.recovered);
        assert!(operation
            .message
            .starts_with("Recovered completed checkpoint:"));
        resumed.finish(true).unwrap();
        assert!(paths.interrupted_operation_journal_path().is_file());
    }

    #[test]
    fn recovery_reexecutes_a_checkpoint_that_no_longer_matches_component_state() {
        let temp = tempfile::tempdir().unwrap();
        let paths = ComponentPaths::for_test(temp.path());
        let component = ComponentId::parse("code").unwrap();
        let mut stale = bundled_operation(&paths);
        stale.version = Some("0.0.0-stale".to_string());
        let mut first = BatchJournal::begin(
            &paths,
            "component.install",
            "install",
            &digest('a'),
            std::slice::from_ref(&component),
        )
        .unwrap();
        first.record_success(&stale).unwrap();
        drop(first);

        let mut resumed = BatchJournal::begin(
            &paths,
            "component.install",
            "install",
            &digest('b'),
            std::slice::from_ref(&component),
        )
        .unwrap();
        assert!(resumed.take_recovered(&component).is_none());
        resumed.record_success(&bundled_operation(&paths)).unwrap();
        resumed.finish(true).unwrap();
    }

    #[test]
    fn corrupt_active_journal_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let paths = ComponentPaths::for_test(temp.path());
        std::fs::create_dir_all(paths.operation_journal_root()).unwrap();
        std::fs::write(paths.active_operation_journal_path(), b"not-json").unwrap();
        let error = BatchJournal::begin(
            &paths,
            "component.install",
            "install",
            &digest('a'),
            &[ComponentId::parse("code").unwrap()],
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("invalid"), "{error:#}");
        assert!(paths.active_operation_journal_path().is_file());
    }
}
