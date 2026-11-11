#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use chrono::{Datelike, Duration as DateDuration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use slint::{ModelRc, SharedString, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    fs,
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

slint::include_modules!();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskRecord {
    id: u64,
    date: String,
    title: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    completed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct TaskStore {
    tasks: Vec<TaskRecord>,
    next_id: u64,
}

impl TaskStore {
    fn load(path: &PathBuf) -> Self {
        let mut store = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .unwrap_or_default();
        let max_id = store.tasks.iter().map(|task| task.id).max().unwrap_or(0);
        store.next_id = store.next_id.max(max_id.saturating_add(1)).max(1);
        store
    }

    fn save(&self, path: &PathBuf) {
        let Some(parent) = path.parent() else {
            return;
        };
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("DayOrder: could not create data directory: {error}");
            return;
        }

        let temporary_path = path.with_extension("json.tmp");
        let result = serde_json::to_vec_pretty(self)
            .map_err(std::io::Error::other)
            .and_then(|bytes| fs::write(&temporary_path, bytes))
            .and_then(|()| {
                if path.exists() {
                    fs::remove_file(path)?;
                }
                fs::rename(&temporary_path, path)
            });
        if let Err(error) = result {
            eprintln!("DayOrder: could not save tasks: {error}");
        }
    }
}

struct AppState {
    store: TaskStore,
    selected_date: NaiveDate,
    data_path: PathBuf,
}

impl AppState {
    fn load() -> Self {
        let mut data_path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        data_path.push("DayOrder");
        data_path.push("tasks.json");
        let store = TaskStore::load(&data_path);
        Self {
            store,
            selected_date: Local::now().date_naive(),
            data_path,
        }
    }

    fn save(&self) {
        self.store.save(&self.data_path);
    }
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let month_index = date.year() * 12 + date.month0() as i32 + months;
    let year = month_index.div_euclid(12);
    let month = month_index.rem_euclid(12) as u32 + 1;
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(date);
    let next_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap_or(first + DateDuration::days(31))
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap_or(first + DateDuration::days(31))
    };
    let last_day = (next_first - DateDuration::days(1)).day();
    first.with_day(date.day().min(last_day)).unwrap_or(first)
}

fn weekday_zh(date: NaiveDate) -> &'static str {
    match date.weekday() {
        chrono::Weekday::Mon => "一",
        chrono::Weekday::Tue => "二",
        chrono::Weekday::Wed => "三",
        chrono::Weekday::Thu => "四",
        chrono::Weekday::Fri => "五",
        chrono::Weekday::Sat => "六",
        chrono::Weekday::Sun => "日",
    }
}

fn task_model(tasks: &[&TaskRecord]) -> ModelRc<TaskItem> {
    let items = tasks
        .iter()
        .map(|task| TaskItem {
            id: task.id.min(i32::MAX as u64) as i32,
            title: SharedString::from(task.title.as_str()),
            note: SharedString::from(task.note.as_str()),
            done: task.completed,
        })
        .collect::<Vec<_>>();
    ModelRc::new(VecModel::from(items))
}

fn refresh_ui(main: &DayOrderMain, widget: &TodayWidget, state: &AppState) {
    let selected = state.selected_date;
    let selected_string = selected.format("%Y-%m-%d").to_string();
    let current_month = selected.month();
    let current_year = selected.year();

    let selected_tasks = state
        .store
        .tasks
        .iter()
        .filter(|task| task.date == selected_string)
        .collect::<Vec<_>>();
    let today = Local::now().date_naive();
    let today_string = today.format("%Y-%m-%d").to_string();
    let mut today_tasks = state
        .store
        .tasks
        .iter()
        .filter(|task| task.date == today_string)
        .collect::<Vec<_>>();
    let mut selected_tasks = selected_tasks;
    selected_tasks.sort_by_key(|task| (task.completed, task.id));
    today_tasks.sort_by_key(|task| (task.completed, task.id));

    let mut calendar_days = Vec::with_capacity(42);
    let first_day = NaiveDate::from_ymd_opt(current_year, current_month, 1).unwrap_or(selected);
    let calendar_start = first_day - DateDuration::days(first_day.weekday().num_days_from_monday() as i64);
    for offset in 0..42_i64 {
        let date = calendar_start + DateDuration::days(offset);
        let date_string = date.format("%Y-%m-%d").to_string();
        let day_tasks = state
            .store
            .tasks
            .iter()
            .filter(|task| task.date == date_string)
            .collect::<Vec<_>>();
        calendar_days.push(CalendarDay {
            date: date_string.into(),
            day_label: date.day().to_string().into(),
            in_month: date.month() == current_month && date.year() == current_year,
            selected: date == selected,
            task_count: day_tasks.len().min(i32::MAX as usize) as i32,
            completed_count: day_tasks.iter().filter(|task| task.completed).count().min(i32::MAX as usize) as i32,
        });
    }
    main.set_calendar_days(ModelRc::new(VecModel::from(calendar_days)));
    main.set_task_list(task_model(&selected_tasks));
    main.set_month_label(format!("{current_year}年{current_month:02}月").into());
    main.set_selected_day_label(format!("{}月{}日 · 星期{}", selected.month(), selected.day(), weekday_zh(selected)).into());

    let completed_today = selected_tasks.iter().filter(|task| task.completed).count();
    main.set_task_count_label(format!("{} 项任务 · 完成 {} 项", selected_tasks.len(), completed_today).into());

    let month_tasks = state
        .store
        .tasks
        .iter()
        .filter(|task| task.date.starts_with(&format!("{current_year}-{current_month:02}")))
        .collect::<Vec<_>>();
    let month_done = month_tasks.iter().filter(|task| task.completed).count();
    main.set_month_summary(format!("{} 项 · {} 项完成", month_tasks.len(), month_done).into());

    let year_prefix = format!("{current_year}-");
    let year_tasks = state
        .store
        .tasks
        .iter()
        .filter(|task| task.date.starts_with(&year_prefix))
        .collect::<Vec<_>>();
    let year_done = year_tasks.iter().filter(|task| task.completed).count();
    let active_days = year_tasks
        .iter()
        .map(|task| task.date.as_str())
        .collect::<HashSet<_>>()
        .len();
    let percent = if year_tasks.is_empty() { 0 } else { (year_done * 100 / year_tasks.len()) as i32 };
    main.set_year_completion(percent);
    main.set_year_completion_label(format!("{percent}%").into());
    main.set_year_summary(format!("{} 项任务 · {} 天有记录", year_tasks.len(), active_days).into());

    let today_done = today_tasks.iter().filter(|task| task.completed).count();
    let today_percent = if today_tasks.is_empty() { 0 } else { (today_done * 100 / today_tasks.len()) as i32 };
    widget.set_task_list(task_model(&today_tasks));
    widget.set_date_label(format!("{}月{}日 · 星期{}", today.month(), today.day(), weekday_zh(today)).into());
    widget.set_completion_percent(today_percent);
    widget.set_completion_label(format!("{today_percent}%").into());
    widget.set_completed_count(today_done.min(i32::MAX as usize) as i32);
    widget.set_total_count(today_tasks.len().min(i32::MAX as usize) as i32);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Rc::new(RefCell::new(AppState::load()));
    let main_window = DayOrderMain::new()?;
    let today_widget = TodayWidget::new()?;
    let tray = DayOrderTray::new()?;
    let widget_visible = Rc::new(Cell::new(true));

    refresh_ui(&main_window, &today_widget, &state.borrow());

    {
        let state = state.clone();
        let main = main_window.clone_strong();
        let widget = today_widget.clone_strong();
        main_window.on_select_date(move |date| {
            if let Ok(parsed) = NaiveDate::parse_from_str(date.as_str(), "%Y-%m-%d") {
                state.borrow_mut().selected_date = parsed;
            }
            refresh_ui(&main, &widget, &state.borrow());
        });
    }
    {
        let state = state.clone();
        let main = main_window.clone_strong();
        let widget = today_widget.clone_strong();
        main_window.on_step_month(move |months| {
            let mut state_guard = state.borrow_mut();
            state_guard.selected_date = add_months(state_guard.selected_date, months);
            drop(state_guard);
            refresh_ui(&main, &widget, &state.borrow());
        });
    }
    {
        let state = state.clone();
        let main = main_window.clone_strong();
        let widget = today_widget.clone_strong();
        main_window.on_jump_today(move || {
            state.borrow_mut().selected_date = Local::now().date_naive();
            refresh_ui(&main, &widget, &state.borrow());
        });
    }
    {
        let state = state.clone();
        let main = main_window.clone_strong();
        let widget = today_widget.clone_strong();
        main_window.on_save_task(move |title, note| {
            let title = title.as_str().trim();
            if title.is_empty() {
                return;
            }
            let note = note.as_str().trim().to_owned();
            let editing_id = main.get_editing_task_id().max(0) as u64;
            {
                let mut state = state.borrow_mut();
                if editing_id > 0 {
                    if let Some(task) = state.store.tasks.iter_mut().find(|task| task.id == editing_id) {
                        task.title = title.to_owned();
                        task.note = note;
                    }
                } else {
                    let id = state.store.next_id;
                    state.store.next_id = state.store.next_id.saturating_add(1).max(1);
                    let date = state.selected_date.format("%Y-%m-%d").to_string();
                    state.store.tasks.push(TaskRecord {
                        id,
                        date,
                        title: title.to_owned(),
                        note,
                        completed: false,
                    });
                }
                state.save();
            }
            main.set_editor_title(SharedString::default());
            main.set_editor_note(SharedString::default());
            main.set_editing_task_id(0);
            refresh_ui(&main, &widget, &state.borrow());
        });
    }
    {
        let main = main_window.clone_strong();
        main_window.on_cancel_edit(move || {
            main.set_editor_title(SharedString::default());
            main.set_editor_note(SharedString::default());
            main.set_editing_task_id(0);
        });
    }
    {
        let state = state.clone();
        let main = main_window.clone_strong();
        let widget = today_widget.clone_strong();
        main_window.on_edit_task(move |id| {
            let state = state.borrow();
            if let Some(task) = state.store.tasks.iter().find(|task| task.id == id.max(0) as u64) {
                main.set_editor_title(task.title.clone().into());
                main.set_editor_note(task.note.clone().into());
                main.set_editing_task_id(id);
            }
            refresh_ui(&main, &widget, &state);
        });
    }
    {
        let state = state.clone();
        let main = main_window.clone_strong();
        let widget = today_widget.clone_strong();
        main_window.on_delete_task(move |id| {
            {
                let mut state = state.borrow_mut();
                state.store.tasks.retain(|task| task.id != id.max(0) as u64);
                state.save();
            }
            if main.get_editing_task_id() == id {
                main.set_editor_title(SharedString::default());
                main.set_editor_note(SharedString::default());
                main.set_editing_task_id(0);
            }
            refresh_ui(&main, &widget, &state.borrow());
        });
    }
    {
        let state = state.clone();
        let main = main_window.clone_strong();
        let widget = today_widget.clone_strong();
        main_window.on_toggle_task(move |id| {
            {
                let mut state = state.borrow_mut();
                if let Some(task) = state.store.tasks.iter_mut().find(|task| task.id == id.max(0) as u64) {
                    task.completed = !task.completed;
                }
                state.save();
            }
            refresh_ui(&main, &widget, &state.borrow());
        });
    }
    {
        let widget = today_widget.clone_strong();
        main_window.on_show_widget(move || {
            widget.show().ok();
        });
    }
    {
        let state = state.clone();
        let main = main_window.clone_strong();
        let widget = today_widget.clone_strong();
        today_widget.on_toggle_task(move |id| {
            {
                let mut state = state.borrow_mut();
                if let Some(task) = state.store.tasks.iter_mut().find(|task| task.id == id.max(0) as u64) {
                    task.completed = !task.completed;
                }
                state.save();
            }
            refresh_ui(&main, &widget, &state.borrow());
        });
    }
    {
        let main = main_window.clone_strong();
        today_widget.on_open_main(move || {
            main.show().ok();
        });
    }
    {
        let widget = today_widget.clone_strong();
        let widget_visible = widget_visible.clone();
        today_widget.on_hide_widget(move || {
            widget.hide().ok();
            widget_visible.set(false);
        });
    }
    {
        let widget_visible = widget_visible.clone();
        today_widget.window().on_close_requested(move || {
            widget_visible.set(false);
            slint::CloseRequestResponse::HideWindow
        });
    }
    main_window.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow);

    {
        let main = main_window.clone_strong();
        tray.on_open_main(move || {
            main.show().ok();
        });
    }
    {
        let widget = today_widget.clone_strong();
        let widget_visible = widget_visible.clone();
        tray.on_toggle_widget(move || {
            if widget_visible.get() {
                widget.hide().ok();
                widget_visible.set(false);
            } else {
                widget.show().ok();
                widget_visible.set(true);
            }
        });
    }
    tray.on_quit_app(|| {
        let _ = slint::quit_event_loop();
    });

    main_window.show()?;
    today_widget.show()?;
    tray.show()?;

    let midnight_state = state.clone();
    let midnight_main = main_window.clone_strong();
    let midnight_widget = today_widget.clone_strong();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_secs(30), move || {
        let today = Local::now().date_naive();
        let changed = {
            let mut state = midnight_state.borrow_mut();
            if state.selected_date != today {
                state.selected_date = today;
                true
            } else {
                false
            }
        };
        if changed {
            refresh_ui(&midnight_main, &midnight_widget, &midnight_state.borrow());
        } else {
            refresh_ui(&midnight_main, &midnight_widget, &midnight_state.borrow());
        }
    });

    slint::run_event_loop()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_navigation_clamps_end_of_month() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
        assert_eq!(add_months(date, 1), NaiveDate::from_ymd_opt(2024, 2, 29).unwrap());
        assert_eq!(add_months(date, 2), NaiveDate::from_ymd_opt(2024, 3, 31).unwrap());
    }

    #[test]
    fn month_navigation_crosses_year_boundary() {
        let date = NaiveDate::from_ymd_opt(2025, 12, 15).unwrap();
        assert_eq!(add_months(date, 1), NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(add_months(date, -12), NaiveDate::from_ymd_opt(2024, 12, 15).unwrap());
    }
}
