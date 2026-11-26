use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::{AppError, AppResult};

const LP_TIMEOUT: Duration = Duration::from_secs(15);
pub const MEDIA_OPTION: &str = "Postcard";
pub const PRINT_SCALING: &str = "fill";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterBackend {
    Cups,
    Mock,
}

impl PrinterBackend {
    pub fn label(self) -> &'static str {
        match self {
            PrinterBackend::Cups => "cups",
            PrinterBackend::Mock => "mock",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrinterHealth {
    pub state: String,
    pub reason: Option<String>,
}

impl PrinterHealth {
    pub fn ok(&self) -> bool {
        matches!(self.state.as_str(), "idle" | "printing" | "processing")
    }

    pub fn unconfigured() -> Self {
        Self {
            state: "unconfigured".to_string(),
            reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintOutcome {
    pub job: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterList {
    pub printers: Vec<String>,
    pub default: Option<String>,
}

pub async fn submit(
    backend: PrinterBackend,
    printer: Option<&str>,
    file: &Path,
    orientation: &str,
    mock_dir: &Path,
) -> AppResult<PrintOutcome> {
    match backend {
        PrinterBackend::Mock => {
            tokio::fs::create_dir_all(mock_dir).await?;
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "print.png".to_string());
            let dest = mock_dir.join(format!(
                "{}-{}",
                chrono::Local::now().format("%Y%m%d-%H%M%S-%3f"),
                name
            ));
            tokio::fs::copy(file, &dest).await?;
            Ok(PrintOutcome {
                job: format!("mock-{}", dest.file_name().unwrap().to_string_lossy()),
                file: dest.to_string_lossy().to_string(),
            })
        }
        PrinterBackend::Cups => {
            let fallback;
            let printer = match printer.filter(|p| !p.trim().is_empty()) {
                Some(printer) => printer,
                None => {
                    fallback = default_printer().await.ok_or_else(|| {
                        AppError::print_failed("No printer configured and CUPS has no default.")
                    })?;
                    &fallback
                }
            };
            let requested = if orientation == "landscape" { "4" } else { "3" };
            let output = timeout(
                LP_TIMEOUT,
                Command::new("lp")
                    .arg("-d")
                    .arg(printer)
                    .arg("-o")
                    .arg(format!("media={MEDIA_OPTION}"))
                    .arg("-o")
                    .arg(format!("print-scaling={PRINT_SCALING}"))
                    .arg("-o")
                    .arg(format!("orientation-requested={requested}"))
                    .arg(file)
                    .output(),
            )
            .await
            .map_err(|_| AppError::print_failed("Printer did not respond in time."))??;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                return Err(AppError::print_failed(if stderr.is_empty() {
                    format!("lp exited with {}", output.status)
                } else {
                    stderr
                }));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let job = parse_lp_job(&stdout)
                .ok_or_else(|| AppError::print_failed("Could not read the CUPS job id."))?;
            Ok(PrintOutcome {
                job,
                file: file.to_string_lossy().to_string(),
            })
        }
    }
}

pub fn parse_lp_job(stdout: &str) -> Option<String> {
    let marker = "request id is ";
    let start = stdout.find(marker)? + marker.len();
    let rest = &stdout[start..];
    let job: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    if job.is_empty() {
        None
    } else {
        Some(job)
    }
}

pub async fn status(backend: PrinterBackend, printer: Option<&str>) -> PrinterHealth {
    if backend == PrinterBackend::Mock {
        return PrinterHealth {
            state: "idle".to_string(),
            reason: None,
        };
    }
    let Some(printer) = printer.filter(|p| !p.trim().is_empty()) else {
        return PrinterHealth::unconfigured();
    };
    let output = timeout(
        LP_TIMEOUT,
        Command::new("lpstat").arg("-p").arg(printer).output(),
    )
    .await;
    let Ok(Ok(output)) = output else {
        return PrinterHealth {
            state: "error".to_string(),
            reason: Some("lpstat failed".to_string()),
        };
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_lpstat(&text)
}

pub fn parse_lpstat(text: &str) -> PrinterHealth {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return PrinterHealth {
            state: "error".to_string(),
            reason: Some("printer not found".to_string()),
        };
    }
    let lowered = trimmed.to_lowercase();
    let state = if lowered.contains("is idle") {
        "idle"
    } else if lowered.contains("now printing") {
        "printing"
    } else if lowered.contains("is processing") {
        "processing"
    } else if lowered.contains("disabled") {
        "stopped"
    } else {
        "unknown"
    };
    let reason = trimmed
        .split(" - ")
        .nth(1)
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty());
    PrinterHealth {
        state: state.to_string(),
        reason,
    }
}

pub async fn waiting(backend: PrinterBackend, printer: Option<&str>) -> u32 {
    if backend == PrinterBackend::Mock {
        return 0;
    }
    let fallback;
    let printer = match printer.filter(|p| !p.trim().is_empty()) {
        Some(printer) => printer,
        None => match default_printer().await {
            Some(printer) => {
                fallback = printer;
                &fallback
            }
            None => return 0,
        },
    };
    let output = timeout(
        LP_TIMEOUT,
        Command::new("lpstat").arg("-o").arg(printer).output(),
    )
    .await;
    match output {
        Ok(Ok(output)) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count() as u32,
        _ => 0,
    }
}

pub async fn default_printer() -> Option<String> {
    let output = timeout(LP_TIMEOUT, Command::new("lpstat").arg("-d").output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_default_destination(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse_default_destination(text: &str) -> Option<String> {
    text.split(": ")
        .nth(1)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "no system default destination")
        .map(ToString::to_string)
}

pub async fn list_printers(backend: PrinterBackend) -> PrinterList {
    if backend == PrinterBackend::Mock {
        return PrinterList {
            printers: vec!["Mock".to_string()],
            default: Some("Mock".to_string()),
        };
    }
    let printers = timeout(LP_TIMEOUT, Command::new("lpstat").arg("-e").output())
        .await
        .ok()
        .and_then(Result::ok)
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let default = timeout(LP_TIMEOUT, Command::new("lpstat").arg("-d").output())
        .await
        .ok()
        .and_then(Result::ok)
        .and_then(|output| parse_default_destination(&String::from_utf8_lossy(&output.stdout)));

    PrinterList { printers, default }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lp_request_id() {
        assert_eq!(
            parse_lp_job("request id is CP1500-42 (1 file(s))\n").as_deref(),
            Some("CP1500-42")
        );
        assert_eq!(parse_lp_job("nonsense"), None);
    }

    #[test]
    fn parses_cups_default_destination() {
        assert_eq!(
            parse_default_destination("system default destination: CP1500\n").as_deref(),
            Some("CP1500")
        );
        assert_eq!(
            parse_default_destination("no system default destination\n"),
            None
        );
    }

    #[test]
    fn parses_lpstat_states() {
        assert_eq!(
            parse_lpstat("printer CP1500 is idle.  enabled since Mon").state,
            "idle"
        );
        let stopped = parse_lpstat("printer CP1500 disabled since Mon - Paper out");
        assert_eq!(stopped.state, "stopped");
        assert_eq!(stopped.reason.as_deref(), Some("Paper out"));
        assert_eq!(parse_lpstat("").state, "error");
    }
}
