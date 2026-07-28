use super::*;

pub(super) fn replace_ide_intelligence_request(ide: &mut Ide) -> (u64, CancellationToken) {
    ide.intelligence_cancellation.cancel();
    ide.intelligence_jump_cancellation.cancel();
    let cancellation = CancellationToken::new();
    ide.intelligence_cancellation = cancellation.clone();
    ide.intelligence_request_id = ide.intelligence_request_id.wrapping_add(1);
    (ide.intelligence_request_id, cancellation)
}

pub(super) fn ide_intelligence_request_is_current(ide: &Ide, request_id: u64) -> bool {
    ide.intelligence_request_id == request_id
        && ide
            .intelligence
            .as_ref()
            .is_some_and(|view| view.request_id == request_id)
}

pub(super) fn replace_ide_intelligence_jump_request(ide: &mut Ide) -> (u64, CancellationToken) {
    ide.intelligence_jump_cancellation.cancel();
    let cancellation = CancellationToken::new();
    ide.intelligence_jump_cancellation = cancellation.clone();
    ide.intelligence_jump_request_id = ide.intelligence_jump_request_id.wrapping_add(1);
    (ide.intelligence_jump_request_id, cancellation)
}

pub(super) fn ide_intelligence_jump_request_is_current(
    ide: &Ide,
    request_id: u64,
    jump_request_id: u64,
) -> bool {
    ide_intelligence_request_is_current(ide, request_id)
        && ide.intelligence_jump_request_id == jump_request_id
}
