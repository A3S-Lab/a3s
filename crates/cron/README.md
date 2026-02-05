# a3s-cron

Cron scheduling library for A3S with natural language support.

## Features

- **Standard Cron Syntax**: 5-field cron expressions (minute hour day month weekday)
- **Natural Language**: Parse schedules from English and Chinese
- **Persistence**: JSON file-based storage with pluggable backends
- **CRUD Operations**: Create, pause, resume, update, and remove jobs
- **Execution History**: Track job runs with output and status

## Usage

```rust
use a3s_cron::{CronManager, FileCronStore, parse_natural};

// Parse natural language to cron expression
let cron = parse_natural("every day at 2am")?;  // Returns "0 2 * * *"
let cron = parse_natural("每天凌晨2点")?;        // Returns "0 2 * * *"

// Create a manager with file-based storage
let store = FileCronStore::new("/path/to/storage").await?;
let manager = CronManager::new(store);

// Add a job
manager.add_job("backup", "0 2 * * *", "backup.sh").await?;

// List jobs
let jobs = manager.list_jobs().await?;

// Pause/resume
manager.pause_job(&job_id).await?;
manager.resume_job(&job_id).await?;
```

## Natural Language Support

### English

- `every minute`, `every 5 minutes`
- `every hour`, `every 2 hours`
- `daily at 2am`, `every day at 14:30`
- `weekly on monday at 9am`
- `monthly on the 15th`
- `every weekday at 8am`
- `every weekend at 10am`

### Chinese

- `每分钟`, `每5分钟`
- `每小时`, `每2小时`
- `每天凌晨2点`, `每天下午3点30分`
- `每周一上午9点`
- `每月15号`
- `工作日上午9点`
- `周末上午10点`

## License

MIT
