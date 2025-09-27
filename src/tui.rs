//! TUI 界面模块
//! 使用 ratatui 提供现代化的终端用户界面

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, Paragraph,
    },
    Frame, Terminal,
};

/// 应用程序状态
#[derive(Debug, Clone)]
pub struct AppState {
    pub status: String,
    pub logs: Vec<LogEntry>,
    pub devices: Vec<DeviceInfo>,
    pub download_progress: Option<DownloadProgress>,
    pub version_info: Option<VersionInfo>,
    pub should_quit: bool,
}

/// 日志条目
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

/// 日志级别
#[derive(Debug, Clone)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
    Device,
    Download,
    Launch,
}

/// 设备信息
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub status: String,
}

/// 下载进度信息
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub filename: String,
    pub progress: f64,
    pub downloaded_mb: f64,
    pub total_mb: f64,
    pub speed_mbps: f64,
}

/// 版本信息
#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub local: String,
    pub remote: String,
    pub needs_update: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: "初始化中...".to_string(),
            logs: Vec::new(),
            devices: Vec::new(),
            download_progress: None,
            version_info: None,
            should_quit: false,
        }
    }
}

impl AppState {
    /// 添加日志条目
    pub fn add_log(&mut self, level: LogLevel, message: String) {
        let timestamp = get_timestamp();
        self.logs.push(LogEntry {
            timestamp,
            level,
            message,
        });
        
        // 保持最多100条日志
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    /// 更新状态
    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    /// 更新设备列表
    pub fn update_devices(&mut self, devices: Vec<DeviceInfo>) {
        self.devices = devices;
    }

}

/// TUI 应用程序
pub struct TuiApp {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    state: AppState,
}

impl TuiApp {
    /// 创建新的 TUI 应用程序
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // 设置终端
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let mut state = AppState::default();
        state.set_status("正在初始化...".to_string());

        Ok(Self {
            terminal,
            state,
        })
    }

    /// 使用共享状态运行 TUI 应用程序
    pub async fn run_with_shared_state(&mut self, shared_state: Arc<Mutex<AppState>>) -> Result<(), Box<dyn std::error::Error>> {
        let tick_rate = Duration::from_millis(100); // 提高刷新频率以获得更快响应
        let mut last_tick = Instant::now();

        loop {
            // 从共享状态获取最新数据
            let state_clone = {
                let state = shared_state.lock().await;
                state.clone()
            };

            self.terminal.draw(|f| draw_ui(f, &state_clone))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                let mut state = shared_state.lock().await;
                                state.should_quit = true;
                                break;
                            }
                            KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                let mut state = shared_state.lock().await;
                                state.should_quit = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }

            // 检查是否应该退出
            {
                let state = shared_state.lock().await;
                if state.should_quit {
                    break;
                }
            }
        }

        Ok(())
    }




    /// 获取应用状态的可变引用
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// 获取应用状态的不可变引用
    pub fn state(&self) -> &AppState {
        &self.state
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        // 恢复终端状态
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}

/// 获取当前时间戳
fn get_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    let secs = now.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", (hours + 8) % 24, minutes, seconds) // UTC+8
}

/// 绘制用户界面
fn draw_ui(f: &mut Frame, state: &AppState) {
    let size = f.area();

    // 主布局：标题 + 内容
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // 标题
            Constraint::Min(0),    // 内容
        ])
        .split(size);

    // 绘制标题
    draw_header(f, chunks[0]);

    // 内容布局：左侧（状态+设备） + 右侧（日志）
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // 左侧
            Constraint::Percentage(50), // 右侧
        ])
        .split(chunks[1]);

    // 左侧布局：状态 + 设备 + 下载进度
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // 状态面板
            Constraint::Min(8),     // 设备列表
            Constraint::Length(if state.download_progress.is_some() { 7 } else { 0 }), // 下载进度
        ])
        .split(content_chunks[0]);

    // 绘制各个组件
    draw_status_panel(f, left_chunks[0], state);
    draw_device_list(f, left_chunks[1], state);
    
    if state.download_progress.is_some() {
        draw_download_progress(f, left_chunks[2], state);
    }

    draw_logs(f, content_chunks[1], state);

    // 如果有版本信息，绘制版本对比弹窗
    if let Some(ref version_info) = state.version_info {
        if version_info.needs_update {
            draw_version_popup(f, size, version_info);
        }
    }
}

/// 绘制标题栏
fn draw_header(f: &mut Frame, area: Rect) {
    let title = format!("🚀 SCRCPY 智能启动器 v{} - 按 'q' 或 Ctrl+C 退出", env!("CARGO_PKG_VERSION"));
    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)));
    f.render_widget(header, area);
}

/// 绘制状态面板
fn draw_status_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let status_text = vec![
        Line::from(vec![
            Span::styled("状态: ", Style::default().fg(Color::Yellow)),
            Span::raw(&state.status),
        ]),
        Line::from(vec![
            Span::styled("时间: ", Style::default().fg(Color::Yellow)),
            Span::raw(get_timestamp()),
        ]),
    ];

    let status_panel = Paragraph::new(status_text)
        .block(Block::default()
            .title("📊 系统状态")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)));
    f.render_widget(status_panel, area);
}

/// 绘制设备列表
fn draw_device_list(f: &mut Frame, area: Rect, state: &AppState) {
    let devices: Vec<ListItem> = if state.devices.is_empty() {
        vec![ListItem::new("📱 暂无设备连接")]
    } else {
        state.devices
            .iter()
            .map(|device| {
                ListItem::new(format!("📱 {} - {} ({})", device.name, device.id, device.status))
            })
            .collect()
    };

    let device_list = List::new(devices)
        .block(Block::default()
            .title("📱 设备列表")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta)));
    f.render_widget(device_list, area);
}

/// 绘制下载进度
fn draw_download_progress(f: &mut Frame, area: Rect, state: &AppState) {
    if let Some(ref progress) = state.download_progress {
        let progress_ratio = progress.progress / 100.0;
        let progress_bar = Gauge::default()
            .block(Block::default()
                .title("📥 下载进度")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(progress_ratio)
            .label(format!(
                "{:.1}% ({:.2} MB / {:.2} MB) - {:.2} MB/s",
                progress.progress,
                progress.downloaded_mb,
                progress.total_mb,
                progress.speed_mbps
            ));
        f.render_widget(progress_bar, area);

        // 文件名信息
        let filename_area = Rect {
            x: area.x + 1,
            y: area.y + area.height - 2,
            width: area.width - 2,
            height: 1,
        };
        let filename_text = Paragraph::new(format!("文件: {}", progress.filename))
            .style(Style::default().fg(Color::Gray));
        f.render_widget(filename_text, filename_area);
    }
}

/// 绘制日志面板
fn draw_logs(f: &mut Frame, area: Rect, state: &AppState) {
    let logs: Vec<ListItem> = state.logs
        .iter()
        .rev() // 最新的日志在顶部
        .take(area.height as usize - 2) // 减去边框高度
        .map(|log| {
            let (icon, color) = match log.level {
                LogLevel::Info => ("ℹ️", Color::White),
                LogLevel::Success => ("✅", Color::Green),
                LogLevel::Warning => ("⚠️", Color::Yellow),
                LogLevel::Error => ("❌", Color::Red),
                LogLevel::Device => ("📱", Color::Magenta),
                LogLevel::Download => ("📥", Color::Blue),
                LogLevel::Launch => ("🚀", Color::Cyan),
            };
            
            ListItem::new(format!("[{}] {} {}", log.timestamp, icon, log.message))
                .style(Style::default().fg(color))
        })
        .collect();

    let log_list = List::new(logs)
        .block(Block::default()
            .title("📋 日志记录")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)));
    f.render_widget(log_list, area);
}

/// 绘制版本对比弹窗
fn draw_version_popup(f: &mut Frame, area: Rect, version_info: &VersionInfo) {
    let popup_area = centered_rect(60, 20, area);
    
    f.render_widget(Clear, popup_area);
    
    let version_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("本地版本: ", Style::default().fg(Color::Yellow)),
            Span::raw(&version_info.local),
        ]),
        Line::from(vec![
            Span::styled("远程版本: ", Style::default().fg(Color::Yellow)),
            Span::raw(&version_info.remote),
        ]),
        Line::from(""),
        Line::from("发现新版本！建议更新以获得最新功能。"),
    ];

    let popup = Paragraph::new(version_text)
        .alignment(Alignment::Center)
        .block(Block::default()
            .title("📦 版本检查")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)));
    // 修复渲染区域错误，应在居中弹窗区域绘制
    f.render_widget(popup, popup_area);
}

/// 创建居中的矩形区域
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

