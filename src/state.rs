use chrono::NaiveDate;

#[derive(Clone, Copy, Debug)]
pub struct State {
    status: Status,
    is_first_draw: bool,

    latest_success: Option<UpdateSuccess>,
    latest_warning: Option<UpdateWarning>,

    completed_units: usize,
    total_units: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    // Prologue
    PingProxy,
    FetchCache,
    // Main download
    Working,
    // Epilogue
    Complete,
    Failed,
}

pub type Update = Result<UpdateSuccess, UpdateWarning>;

#[derive(Clone, Copy, Debug)]
pub enum UpdateSuccess {
    // Prologue
    ProxyPing,
    FetchCache,
    // Main download
    FetchUrl { date: NaiveDate },
    FetchImage { date: NaiveDate },
    SaveImage { date: NaiveDate },
    // Epilogue
    Complete,
}

#[derive(Clone, Copy, Debug)]
pub enum UpdateWarning {
    FetchUrl { attempt: usize, date: NaiveDate },
    FetchImage { attempt: usize, date: NaiveDate },
}

impl State {
    pub fn new(total_units: usize) -> Self {
        Self {
            status: Status::PingProxy,
            is_first_draw: true,

            latest_success: None,
            latest_warning: None,

            completed_units: 0,
            total_units,
        }
    }

    pub fn set_failed(&mut self) {
        self.status = Status::Failed;
        self.latest_success = None;
    }

    pub fn update(&mut self, update: Update) {
        match update {
            Ok(success) => {
                self.latest_success = Some(success);
                match success {
                    UpdateSuccess::ProxyPing => self.status = Status::FetchCache,
                    UpdateSuccess::FetchCache => self.status = Status::Working,
                    UpdateSuccess::Complete => self.status = Status::Complete,

                    UpdateSuccess::SaveImage { .. } => {
                        self.increase_complete_units();
                    }
                    _ => (),
                }
            }
            Err(warning) => {
                self.latest_warning = Some(warning);
            }
        }
    }

    fn increase_complete_units(&mut self) {
        if self.status != Status::Working {
            return;
        }
        if self.completed_units < self.total_units {
            self.completed_units += 1;
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn latest_success(&self) -> Option<UpdateSuccess> {
        self.latest_success
    }

    pub fn latest_warning(&self) -> Option<UpdateWarning> {
        self.latest_warning
    }

    pub fn completed_units(&self) -> usize {
        self.completed_units
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
