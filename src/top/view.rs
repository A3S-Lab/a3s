//! Shared process-table renderer used by the `a3s top` process surfaces. The
//! rich sparkline columns degrade to blank cells when a caller has no per-pid
//! history.

use std::collections::HashSet;

use a3s_tui::components::{CellAlign, DataColumn, DataRow, DataTable, Sparkline};
use a3s_tui::style::Color;

use super::{display_workspace, ProcessRow, YELLOW};

/// Per-pid `(cpu, mem)` history lookup feeding the sparkline columns.
pub(crate) type HistoryFn<'a> = dyn Fn(u32) -> (Vec<f32>, Vec<f32>) + 'a;

/// View parameters for [`render_process_table`].
pub(crate) struct ProcessTableView<'a> {
    pub(crate) selected: usize,
    pub(crate) scroll: usize,
    pub(crate) width: u16,
    pub(crate) height: usize,
    /// Column ids to hide (empty = show all).
    pub(crate) hidden: &'a HashSet<String>,
    /// Per-pid history feeding the sparkline columns. `None` renders those cells
    /// blank when a caller has no retained history.
    pub(crate) history: Option<&'a HistoryFn<'a>>,
}

fn configured(hidden: &HashSet<String>, id: &str, column: DataColumn) -> DataColumn {
    if hidden.contains(id) {
        column.hidden()
    } else {
        column
    }
}

fn sparkline(values: &[f32], color: Color) -> String {
    Sparkline::new(values.iter().copied().map(f64::from))
        .width(8)
        .range(0.0, 100.0)
        .fg(color)
        .view()
}

/// Build the host process table. Agent rows wear their brand colour; other
/// rows are coloured by risk. The selected row is highlighted by the table.
fn process_data_table(rows: &[ProcessRow], view: &ProcessTableView) -> DataTable {
    let h = view.hidden;
    let columns = vec![
        configured(
            h,
            "processes.pid",
            DataColumn::new("PID").width(7).align(CellAlign::Right),
        ),
        configured(
            h,
            "processes.ppid",
            DataColumn::new("PPID").width(7).align(CellAlign::Right),
        ),
        configured(
            h,
            "processes.cpu",
            DataColumn::new("CPU%").width(6).align(CellAlign::Right),
        ),
        configured(h, "processes.cpu_history", DataColumn::new("CPU").width(8)),
        configured(
            h,
            "processes.mem",
            DataColumn::new("MEM%").width(6).align(CellAlign::Right),
        ),
        configured(h, "processes.mem_history", DataColumn::new("MEM").width(8)),
        configured(h, "processes.risk", DataColumn::new("RISK").width(5)),
        configured(h, "processes.elapsed", DataColumn::new("ELAPSED").width(9)),
        configured(h, "processes.cwd", DataColumn::new("CWD").width(18)),
        configured(
            h,
            "processes.command",
            DataColumn::new("COMMAND").min_width(16),
        ),
    ];

    let mut table = DataTable::new(columns)
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .selected((!rows.is_empty()).then_some(view.selected))
        .scroll(view.scroll)
        .empty("no diagnostic processes match the current filter");

    for row in rows {
        let color = row
            .agent
            .map(|a| a.color())
            .unwrap_or_else(|| row.risk.color());
        let (cpu_hist, mem_hist) = match view.history {
            Some(history) => history(row.pid),
            None => (Vec::new(), Vec::new()),
        };
        let cells = vec![
            row.pid.to_string(),
            row.ppid.to_string(),
            format!("{:.1}", row.cpu_pct),
            sparkline(&cpu_hist, color),
            format!("{:.1}", row.mem_pct),
            sparkline(&mem_hist, YELLOW),
            row.risk.label().to_string(),
            row.elapsed.clone(),
            display_workspace(row.cwd.as_deref()),
            row.command.clone(),
        ];
        table.add_row(DataRow::new(cells).fg(color));
    }

    table
}

/// Render the host process table.
pub(crate) fn render_process_table(rows: &[ProcessRow], view: &ProcessTableView) -> String {
    let table = process_data_table(rows, view);
    table.view(view.width, view.height)
}
