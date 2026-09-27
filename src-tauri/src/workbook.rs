//! Excel workbook generation for the `create_workbook` agent tool.
//!
//! The model describes a workbook as JSON ([`WorkbookSpec`]) and this module
//! builds a real `.xlsx` in memory with `rust_xlsxwriter`: typed cells, live
//! formulas (Excel recalculates on open), number formats, Excel tables,
//! frozen headers, drop-down validation, conditional formatting, charts and
//! named ranges. Building is pure (no I/O); the bytes are written through
//! `security::files::write_bytes`, so the path policy, native confirmation
//! and audit apply exactly as for any other file write.
//!
//! The spec is model output, i.e. untrusted: every size is bounded, ranges are
//! parsed strictly, and errors are returned to the model so it can fix them.

use crate::error::{AppError, AppResult};
use rust_xlsxwriter::{
    Chart, ChartType, ConditionalFormat3ColorScale, ConditionalFormatDataBar,
    ConditionalFormatFormula, DataValidation, DataValidationRule, DocProperties, Format,
    FormatAlign, FormatBorder, Formula, Table, TableColumn, TableStyle, Workbook, Worksheet,
    XlsxError,
};
use serde::Deserialize;
use serde_json::Value;

/// Most sheets in one workbook.
pub const MAX_SHEETS: usize = 50;
/// Most cells (rows × columns, all sheets) the tool will write.
pub const MAX_CELLS: usize = 200_000;
/// Most charts, conditional formats or validations per sheet.
const MAX_EXTRAS: usize = 50;
/// Excel's limits.
const MAX_ROW: u32 = 1_048_575;
const MAX_COL: u16 = 16_383;

/// A workbook as described by the model.
#[derive(Debug, Deserialize)]
pub struct WorkbookSpec {
    /// Document title (File → Info).
    #[serde(default)]
    pub title: Option<String>,
    /// Sheets in tab order.
    pub sheets: Vec<SheetSpec>,
    /// Workbook-level named ranges, e.g. `{"name": "Rates", "range": "Lookup!$A$2:$B$20"}`.
    #[serde(default)]
    pub named_ranges: Vec<NamedRange>,
}

/// A workbook-level defined name.
#[derive(Debug, Deserialize)]
pub struct NamedRange {
    /// Name used in formulas.
    pub name: String,
    /// Absolute reference, e.g. `Sheet1!$A$2:$A$50`.
    pub range: String,
}

/// One worksheet.
#[derive(Debug, Deserialize)]
pub struct SheetSpec {
    /// Tab name (max 31 characters, no `[]:*?/\`).
    pub name: String,
    /// Header row. When present, data starts on row 2.
    #[serde(default)]
    pub columns: Vec<ColumnSpec>,
    /// Data rows. Strings starting with `=` are formulas; `null` is blank.
    #[serde(default)]
    pub rows: Vec<Vec<Value>>,
    /// Format header + rows as an Excel table (filter buttons, banding,
    /// structured references). Default: true when there are columns.
    #[serde(default)]
    pub table: Option<bool>,
    /// Table style, e.g. `Medium2`, `Light9`, `Dark1`.
    #[serde(default)]
    pub table_style: Option<String>,
    /// Add a total row to the table (sums numeric columns with `total`).
    #[serde(default)]
    pub total_row: bool,
    /// Freeze the header row (default true when there are columns).
    #[serde(default)]
    pub freeze_header: Option<bool>,
    /// Tab colour, `#RRGGBB`.
    #[serde(default)]
    pub tab_color: Option<String>,
    /// Print in landscape, fit to one page wide.
    #[serde(default)]
    pub landscape: bool,
    /// Conditional formats.
    #[serde(default)]
    pub conditional_formats: Vec<CondSpec>,
    /// Data validation (drop-downs, number limits).
    #[serde(default)]
    pub validations: Vec<ValidationSpec>,
    /// Charts.
    #[serde(default)]
    pub charts: Vec<ChartSpec>,
}

/// A header column.
#[derive(Debug, Deserialize)]
pub struct ColumnSpec {
    /// Header text.
    pub header: String,
    /// Width in characters (default: fitted to the header and data).
    #[serde(default)]
    pub width: Option<f64>,
    /// Excel number format for the column's data, e.g. `$#,##0.00`, `0.0%`, `yyyy-mm-dd`.
    #[serde(default)]
    pub format: Option<String>,
    /// Table total-row function: `sum`, `average`, `count`, `min`, `max`.
    #[serde(default)]
    pub total: Option<String>,
}

/// A conditional format over an A1 range.
#[derive(Debug, Deserialize)]
pub struct CondSpec {
    /// Target range, e.g. `C2:C200`.
    pub range: String,
    /// `formula`, `data_bar` or `color_scale`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `formula`: rule relative to the range's top-left cell, e.g. `=$C2<0`.
    #[serde(default)]
    pub formula: Option<String>,
    /// `formula`: fill colour (`#RRGGBB`). `data_bar`: bar colour.
    #[serde(default)]
    pub fill_color: Option<String>,
    /// `formula`: font colour.
    #[serde(default)]
    pub font_color: Option<String>,
    /// `formula`: bold text.
    #[serde(default)]
    pub bold: bool,
}

/// Data validation over an A1 range.
#[derive(Debug, Deserialize)]
pub struct ValidationSpec {
    /// Target range, e.g. `D2:D500`.
    pub range: String,
    /// Allowed values (drop-down list).
    #[serde(default)]
    pub list: Vec<String>,
    /// Numeric minimum (inclusive).
    #[serde(default)]
    pub min: Option<f64>,
    /// Numeric maximum (inclusive).
    #[serde(default)]
    pub max: Option<f64>,
    /// Whole numbers only.
    #[serde(default)]
    pub whole: bool,
    /// Prompt shown when the cell is selected.
    #[serde(default)]
    pub input_message: Option<String>,
}

/// A chart placed on the sheet.
#[derive(Debug, Deserialize)]
pub struct ChartSpec {
    /// `column`, `bar`, `line`, `pie`, `doughnut`, `area`, `scatter`,
    /// `column_stacked`, `bar_stacked`, `radar`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Chart title.
    #[serde(default)]
    pub title: Option<String>,
    /// Category range, e.g. `Sales!$A$2:$A$13`.
    #[serde(default)]
    pub categories: Option<String>,
    /// Series (at least one).
    pub series: Vec<SeriesSpec>,
    /// Top-left anchor cell, e.g. `H2`.
    #[serde(default)]
    pub cell: Option<String>,
    /// X axis title.
    #[serde(default)]
    pub x_title: Option<String>,
    /// Y axis title.
    #[serde(default)]
    pub y_title: Option<String>,
}

/// One chart series.
#[derive(Debug, Deserialize)]
pub struct SeriesSpec {
    /// Legend name.
    #[serde(default)]
    pub name: Option<String>,
    /// Value range, e.g. `Sales!$B$2:$B$13`.
    pub values: String,
}

/// Parse and validate the model's JSON.
pub fn parse(v: &Value) -> AppResult<WorkbookSpec> {
    let spec: WorkbookSpec = serde_json::from_value(v.clone())
        .map_err(|e| AppError::InvalidInput(format!("invalid workbook spec: {e}")))?;
    if spec.sheets.is_empty() || spec.sheets.len() > MAX_SHEETS {
        return Err(AppError::InvalidInput(format!(
            "a workbook needs 1–{MAX_SHEETS} sheets"
        )));
    }
    let cells: usize = spec
        .sheets
        .iter()
        .map(|s| {
            let width = s
                .rows
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or(0)
                .max(s.columns.len());
            (s.rows.len() + 1) * width
        })
        .sum();
    if cells > MAX_CELLS {
        return Err(AppError::InvalidInput(format!(
            "the workbook has {cells} cells; the limit is {MAX_CELLS}"
        )));
    }
    for s in &spec.sheets {
        if s.conditional_formats.len() > MAX_EXTRAS
            || s.validations.len() > MAX_EXTRAS
            || s.charts.len() > MAX_EXTRAS
        {
            return Err(AppError::InvalidInput(format!(
                "sheet `{}`: at most {MAX_EXTRAS} charts, conditional formats or validations",
                s.name
            )));
        }
    }
    Ok(spec)
}

/// Build the `.xlsx` bytes.
pub fn build(spec: &WorkbookSpec) -> AppResult<Vec<u8>> {
    let mut wb = Workbook::new();
    let mut props = DocProperties::new().set_author("OMNIX");
    if let Some(t) = spec.title.as_deref().filter(|t| !t.trim().is_empty()) {
        props = props.set_title(t);
    }
    wb.set_properties(&props);
    for sheet in &spec.sheets {
        let ws = wb.add_worksheet();
        write_sheet(ws, sheet).map_err(|e| sheet_err(&sheet.name, e))?;
    }
    for n in &spec.named_ranges {
        let range = if n.range.starts_with('=') {
            n.range.clone()
        } else {
            format!("={}", n.range)
        };
        wb.define_name(&n.name, &range)
            .map_err(|e| AppError::InvalidInput(format!("named range `{}`: {e}", n.name)))?;
    }
    wb.save_to_buffer()
        .map_err(|e| AppError::Internal(format!("cannot build workbook: {e}")))
}

fn sheet_err(name: &str, e: AppError) -> AppError {
    match e {
        AppError::InvalidInput(m) => AppError::InvalidInput(format!("sheet `{name}`: {m}")),
        other => other,
    }
}

fn xl(e: XlsxError) -> AppError {
    AppError::InvalidInput(e.to_string())
}

fn write_sheet(ws: &mut Worksheet, s: &SheetSpec) -> AppResult<()> {
    ws.set_name(&s.name).map_err(xl)?;
    if let Some(c) = s.tab_color.as_deref() {
        ws.set_tab_color(color(c)?);
    }
    if s.landscape {
        ws.set_landscape().set_print_fit_to_pages(1, 0);
    }
    let has_header = !s.columns.is_empty();
    let first_data_row: u32 = u32::from(has_header);
    let width = s
        .rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(s.columns.len());
    if width > usize::from(MAX_COL) + 1 {
        return Err(AppError::InvalidInput("too many columns".into()));
    }
    let col_formats: Vec<Option<Format>> = s
        .columns
        .iter()
        .map(|c| {
            c.format
                .as_deref()
                .filter(|f| !f.is_empty())
                .map(|f| Format::new().set_num_format(f))
        })
        .collect();

    // Data cells.
    for (r, row) in s.rows.iter().enumerate() {
        let r = first_data_row + u32::try_from(r).unwrap_or(MAX_ROW);
        if r > MAX_ROW {
            return Err(AppError::InvalidInput("too many rows".into()));
        }
        for (c, v) in row.iter().enumerate() {
            let c = c as u16;
            let fmt = col_formats.get(usize::from(c)).and_then(Option::as_ref);
            write_cell(ws, r, c, v, fmt)?;
        }
    }

    let last_row = first_data_row + (s.rows.len() as u32).max(1) - 1 + u32::from(s.total_row);
    let use_table = has_header && s.table.unwrap_or(true);
    if has_header {
        let last_col = (width.max(1) - 1) as u16;
        if use_table {
            let columns: Vec<TableColumn> = s
                .columns
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let mut tc = TableColumn::new().set_header(&c.header);
                    if s.total_row && !s.rows.is_empty() {
                        let data = rust_xlsxwriter::utility::cell_range(
                            first_data_row,
                            i as u16,
                            last_row - 1,
                            i as u16,
                        );
                        if let Some(f) = c.total.as_deref().and_then(|t| total_fn(t, &data)) {
                            tc = tc.set_total_function(f);
                        }
                    }
                    tc
                })
                .collect();
            let table = Table::new()
                .set_columns(&columns)
                .set_total_row(s.total_row)
                .set_style(table_style(s.table_style.as_deref()));
            ws.add_table(0, 0, last_row, last_col, &table).map_err(xl)?;
        } else {
            let header = Format::new()
                .set_bold()
                .set_background_color("#1F4E78")
                .set_font_color("#FFFFFF")
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Center)
                .set_text_wrap();
            for (c, col) in s.columns.iter().enumerate() {
                ws.write_string_with_format(0, c as u16, &col.header, &header)
                    .map_err(xl)?;
            }
            ws.autofilter(0, 0, last_row, last_col).map_err(xl)?;
        }
        if s.freeze_header.unwrap_or(true) {
            ws.set_freeze_panes(1, 0).map_err(xl)?;
        }
    }

    // Widths: explicit, else fitted to the header and data.
    ws.autofit();
    for (c, col) in s.columns.iter().enumerate() {
        let w = col
            .width
            .unwrap_or_else(|| {
                let longest = s
                    .rows
                    .iter()
                    .filter_map(|r| r.get(c))
                    .map(|v| match v {
                        Value::String(t) if !t.starts_with('=') => t.chars().count(),
                        Value::Number(n) => n.to_string().len() + 2,
                        _ => 10,
                    })
                    .chain(std::iter::once(col.header.chars().count() + 4))
                    .max()
                    .unwrap_or(10);
                longest.clamp(8, 60) as f64
            })
            .clamp(1.0, 255.0);
        ws.set_column_width(c as u16, w).map_err(xl)?;
    }

    for cf in &s.conditional_formats {
        let (r1, c1, r2, c2) = parse_range(&cf.range)?;
        match cf.kind.as_str() {
            "formula" => {
                let rule = cf.formula.as_deref().ok_or_else(|| {
                    AppError::InvalidInput("conditional format `formula` needs `formula`".into())
                })?;
                let mut f = Format::new();
                if let Some(c) = cf.fill_color.as_deref() {
                    f = f.set_background_color(color(c)?);
                }
                if let Some(c) = cf.font_color.as_deref() {
                    f = f.set_font_color(color(c)?);
                }
                if cf.bold {
                    f = f.set_bold();
                }
                let fmt = ConditionalFormatFormula::new()
                    .set_rule(Formula::new(rule))
                    .set_format(f);
                ws.add_conditional_format(r1, c1, r2, c2, &fmt)
                    .map_err(xl)?;
            }
            "data_bar" => {
                let mut bar = ConditionalFormatDataBar::new();
                if let Some(c) = cf.fill_color.as_deref() {
                    bar = bar.set_fill_color(color(c)?);
                }
                ws.add_conditional_format(r1, c1, r2, c2, &bar)
                    .map_err(xl)?;
            }
            "color_scale" => {
                ws.add_conditional_format(r1, c1, r2, c2, &ConditionalFormat3ColorScale::new())
                    .map_err(xl)?;
            }
            other => {
                return Err(AppError::InvalidInput(format!(
                    "unknown conditional format type `{other}` (formula, data_bar, color_scale)"
                )))
            }
        }
    }

    for v in &s.validations {
        let (r1, c1, r2, c2) = parse_range(&v.range)?;
        let mut dv = if !v.list.is_empty() {
            DataValidation::new()
                .allow_list_strings(&v.list)
                .map_err(xl)?
        } else {
            match (v.min, v.max, v.whole) {
                (Some(lo), Some(hi), true) => DataValidation::new()
                    .allow_whole_number(DataValidationRule::Between(lo as i32, hi as i32)),
                (Some(lo), None, true) => DataValidation::new()
                    .allow_whole_number(DataValidationRule::GreaterThanOrEqualTo(lo as i32)),
                (None, Some(hi), true) => DataValidation::new()
                    .allow_whole_number(DataValidationRule::LessThanOrEqualTo(hi as i32)),
                (Some(lo), Some(hi), false) => {
                    DataValidation::new().allow_decimal_number(DataValidationRule::Between(lo, hi))
                }
                (Some(lo), None, false) => DataValidation::new()
                    .allow_decimal_number(DataValidationRule::GreaterThanOrEqualTo(lo)),
                (None, Some(hi), false) => DataValidation::new()
                    .allow_decimal_number(DataValidationRule::LessThanOrEqualTo(hi)),
                (None, None, _) => {
                    return Err(AppError::InvalidInput(format!(
                        "validation on {} needs `list`, `min` or `max`",
                        v.range
                    )))
                }
            }
        };
        if let Some(m) = v.input_message.as_deref().filter(|m| !m.is_empty()) {
            dv = dv.set_input_message(m).map_err(xl)?;
        }
        ws.add_data_validation(r1, c1, r2, c2, &dv).map_err(xl)?;
    }

    for (i, cs) in s.charts.iter().enumerate() {
        let chart = build_chart(cs)?;
        let (row, col) = match cs.cell.as_deref() {
            Some(cell) => parse_cell(cell)?,
            // Default: to the right of the data, stacked 16 rows apart.
            None => (1 + 16 * i as u32, (width as u16).saturating_add(1)),
        };
        ws.insert_chart(row, col, &chart).map_err(xl)?;
    }
    Ok(())
}

/// Write one JSON value as a typed cell.
fn write_cell(
    ws: &mut Worksheet,
    r: u32,
    c: u16,
    v: &Value,
    fmt: Option<&Format>,
) -> AppResult<()> {
    let res = match (v, fmt) {
        (Value::Null, _) => return Ok(()),
        (Value::Bool(b), _) => ws.write_boolean(r, c, *b),
        (Value::Number(n), Some(f)) => {
            ws.write_number_with_format(r, c, n.as_f64().unwrap_or(0.0), f)
        }
        (Value::Number(n), None) => ws.write_number(r, c, n.as_f64().unwrap_or(0.0)),
        (Value::String(t), f) if t.starts_with('=') && t.len() > 1 => match f {
            Some(f) => ws.write_formula_with_format(r, c, Formula::new(t), f),
            None => ws.write_formula(r, c, Formula::new(t)),
        },
        (Value::String(t), Some(f)) => ws.write_string_with_format(r, c, t, f),
        (Value::String(t), None) => ws.write_string(r, c, t),
        (other, _) => ws.write_string(r, c, other.to_string()),
    };
    res.map(|_| ()).map_err(xl)
}

fn build_chart(cs: &ChartSpec) -> AppResult<Chart> {
    let kind = match cs.kind.as_str() {
        "column" => ChartType::Column,
        "column_stacked" => ChartType::ColumnStacked,
        "bar" => ChartType::Bar,
        "bar_stacked" => ChartType::BarStacked,
        "line" => ChartType::Line,
        "pie" => ChartType::Pie,
        "doughnut" => ChartType::Doughnut,
        "area" => ChartType::Area,
        "scatter" => ChartType::Scatter,
        "radar" => ChartType::Radar,
        other => {
            return Err(AppError::InvalidInput(format!(
                "unknown chart type `{other}`"
            )))
        }
    };
    if cs.series.is_empty() {
        return Err(AppError::InvalidInput(
            "a chart needs at least one series".into(),
        ));
    }
    let mut chart = Chart::new(kind);
    for s in &cs.series {
        let values = sheet_range(&s.values)?;
        let series = chart.add_series();
        series.set_values(values.as_str());
        if let Some(c) = cs.categories.as_deref() {
            series.set_categories(sheet_range(c)?.as_str());
        }
        if let Some(n) = s.name.as_deref() {
            series.set_name(n);
        }
    }
    if let Some(t) = cs.title.as_deref() {
        chart.title().set_name(t);
    }
    if let Some(t) = cs.x_title.as_deref() {
        chart.x_axis().set_name(t);
    }
    if let Some(t) = cs.y_title.as_deref() {
        chart.y_axis().set_name(t);
    }
    Ok(chart)
}

/// Chart ranges must name their sheet (`Sheet!$A$2:$A$9`).
fn sheet_range(r: &str) -> AppResult<String> {
    let r = r.trim().trim_start_matches('=');
    if !r.contains('!') {
        return Err(AppError::InvalidInput(format!(
            "chart range `{r}` must include the sheet, e.g. Sales!$B$2:$B$13"
        )));
    }
    Ok(r.to_string())
}

/// Total-row formula over the column's data. Plain A1 ranges, not the
/// structured references Excel's built-in totals use, so LibreOffice (which
/// shows `#NAME?` for those) computes them too. 1xx codes skip filtered rows.
fn total_fn(name: &str, data: &str) -> Option<rust_xlsxwriter::TableFunction> {
    let code = match name.to_ascii_lowercase().as_str() {
        "sum" => 109,
        "average" | "avg" => 101,
        "count" => 103,
        "min" => 105,
        "max" => 104,
        _ => return None,
    };
    Some(rust_xlsxwriter::TableFunction::Custom(Formula::new(
        format!("SUBTOTAL({code},{data})"),
    )))
}

fn table_style(name: Option<&str>) -> TableStyle {
    use TableStyle as T;
    match name.unwrap_or("Medium2").to_ascii_lowercase().as_str() {
        "none" => T::None,
        "light1" => T::Light1,
        "light9" => T::Light9,
        "light15" => T::Light15,
        "medium1" => T::Medium1,
        "medium4" => T::Medium4,
        "medium7" => T::Medium7,
        "medium9" => T::Medium9,
        "medium16" => T::Medium16,
        "dark1" => T::Dark1,
        "dark9" => T::Dark9,
        _ => T::Medium2,
    }
}

/// `#RRGGBB` (the `#` is optional).
fn color(s: &str) -> AppResult<rust_xlsxwriter::Color> {
    let hex = s.trim().trim_start_matches('#');
    u32::from_str_radix(hex, 16)
        .ok()
        .filter(|_| hex.len() == 6)
        .map(rust_xlsxwriter::Color::RGB)
        .ok_or_else(|| AppError::InvalidInput(format!("colour `{s}` must be #RRGGBB")))
}

/// Parse an A1 cell (`$` allowed) into zero-based (row, col).
fn parse_cell(s: &str) -> AppResult<(u32, u16)> {
    let t = s.trim().replace('$', "").to_ascii_uppercase();
    let split = t.find(|c: char| c.is_ascii_digit()).unwrap_or(t.len());
    let (letters, digits) = t.split_at(split);
    let bad = || AppError::InvalidInput(format!("`{s}` is not a cell reference like B2"));
    if letters.is_empty() || letters.len() > 3 || !letters.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(bad());
    }
    let col = letters
        .chars()
        .fold(0u32, |acc, c| acc * 26 + (c as u32 - 'A' as u32 + 1))
        - 1;
    let row: u32 = digits.parse().map_err(|_| bad())?;
    if row == 0 || row - 1 > MAX_ROW || col > u32::from(MAX_COL) {
        return Err(bad());
    }
    Ok((row - 1, col as u16))
}

/// Parse `A1:C10` (or a single cell) into zero-based bounds.
fn parse_range(s: &str) -> AppResult<(u32, u16, u32, u16)> {
    let s = s.rsplit('!').next().unwrap_or(s);
    let (a, b) = s.split_once(':').unwrap_or((s, s));
    let (r1, c1) = parse_cell(a)?;
    let (r2, c2) = parse_cell(b)?;
    Ok((r1.min(r2), c1.min(c2), r1.max(r2), c1.max(c2)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_cells_and_ranges() {
        assert_eq!(parse_cell("A1").expect("a1"), (0, 0));
        assert_eq!(parse_cell("$AB$12").expect("ab12"), (11, 27));
        assert_eq!(parse_range("Sheet1!C10:A2").expect("range"), (1, 0, 9, 2));
        for bad in ["", "1A", "A0", "ABCD1", "A", "é1"] {
            assert!(parse_cell(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn builds_a_full_workbook() {
        let spec = parse(&json!({
            "title": "Budget",
            "sheets": [{
                "name": "Budget",
                "columns": [
                    {"header": "Month"},
                    {"header": "Planned", "format": "$#,##0.00", "total": "sum"},
                    {"header": "Actual", "format": "$#,##0.00", "total": "sum"},
                    {"header": "Variance", "format": "$#,##0.00"},
                    {"header": "Status"}
                ],
                "rows": [
                    ["Jan", 1000, 950, "=C2-B2", "OK"],
                    ["Feb", 1000, 1200, "=C3-B3", "Over"],
                    ["Mar", 1000, null, "=XLOOKUP(\"Jan\",A2:A4,B2:B4)", "OK"]
                ],
                "total_row": true,
                "conditional_formats": [
                    {"range": "D2:D4", "type": "formula", "formula": "=$D2>0", "fill_color": "#FFC7CE"},
                    {"range": "C2:C4", "type": "data_bar"}
                ],
                "validations": [
                    {"range": "E2:E100", "list": ["OK", "Over", "Under"], "input_message": "Pick one"},
                    {"range": "B2:B100", "min": 0, "max": 100000}
                ],
                "charts": [{
                    "type": "column", "title": "Planned vs actual",
                    "categories": "Budget!$A$2:$A$4",
                    "series": [
                        {"name": "Planned", "values": "Budget!$B$2:$B$4"},
                        {"name": "Actual", "values": "Budget!$C$2:$C$4"}
                    ]
                }]
            }, {
                "name": "Lookup",
                "table": false,
                "columns": [{"header": "Code"}, {"header": "Rate", "format": "0.0%"}],
                "rows": [["A", 0.05], ["B", 0.075]]
            }],
            "named_ranges": [{"name": "Rates", "range": "Lookup!$A$2:$B$3"}]
        }))
        .expect("spec");
        let bytes = build(&spec).expect("build");
        // A zip (xlsx) container.
        assert_eq!(&bytes[..2], b"PK");
        assert!(bytes.len() > 4_000);
    }

    #[test]
    fn rejects_bad_specs() {
        assert!(parse(&json!({"sheets": []})).is_err());
        let too_big = json!({"sheets": [{"name": "S", "rows": vec![vec![1; 1000]; 300]}]});
        assert!(parse(&too_big).is_err());
        let spec = parse(&json!({"sheets": [{"name": "S", "charts": [
            {"type": "column", "series": [{"values": "$B$2:$B$4"}]}
        ]}]}))
        .expect("parse");
        assert!(build(&spec).is_err(), "chart range without a sheet");
        let spec = parse(&json!({"sheets": [{"name": "bad/name"}]})).expect("parse");
        assert!(build(&spec).is_err());
    }
}
