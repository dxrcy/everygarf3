use chrono::NaiveDate;

#[derive(Clone, Copy, Debug)]
pub struct State {
    status: Status,
    latest_update: Option<Update>,
    total_units: usize,
    is_first_draw: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Status {
    Prologue,
    Epilogue,
    Working { completed_units: usize },
}

#[derive(Clone, Copy, Debug)]
pub enum Update {
    FetchUrlSuccess { date: NaiveDate },
    FetchImageSuccess { date: NaiveDate },
    SaveImageSuccess { date: NaiveDate },
}

impl State {
    pub fn new(total_units: usize) -> Self {
        Self {
            status: Status::Prologue,
            latest_update: None,
            total_units,
            is_first_draw: true,
        }
    }

    pub fn advance_state(&mut self) {
        match self.status {
            Status::Prologue => self.status = Status::Working { completed_units: 0 },
            Status::Working { .. } => self.status = Status::Epilogue,
            Status::Epilogue => (),
        }
    }

    pub fn update(&mut self, update: Update) {
        self.latest_update = Some(update);
        if matches!(update, Update::SaveImageSuccess { .. }) {
            self.increase_complete_units();
        }
    }

    fn increase_complete_units(&mut self) {
        let Status::Working {
            ref mut completed_units,
        } = self.status
        else {
            return;
        };
        if *completed_units < self.total_units {
            *completed_units += 1;
        }
    }

    pub fn completed_units(&self) -> usize {
        match self.status {
            Status::Prologue => 0,
            Status::Epilogue => self.total_units,
            Status::Working { completed_units } => completed_units,
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn latest_update(&self) -> Option<Update> {
        self.latest_update
    }

    pub fn total_units(&self) -> usize {
        self.total_units
    }

    pub fn record_draw(&mut self) -> bool {
        let was_first_draw = self.is_first_draw;
        self.is_first_draw = false;
        was_first_draw
    }
}
