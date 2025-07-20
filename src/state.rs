use chrono::NaiveDate;

#[derive(Clone, Copy, Debug)]
pub struct State {
    status: Status,
    latest_update: Option<UpdateResult>,
    is_first_draw: bool,

    completed_units: usize,
    total_units: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    PingProxy,
    Working,
    Epilogue,
    Failed,
}

pub type UpdateResult = Result<UpdateOk, UpdateErr>;

#[derive(Clone, Copy, Debug)]
pub enum UpdateOk {
    ProxyPing,

    FetchUrl { date: NaiveDate },
    FetchImage { date: NaiveDate },
    SaveImage { date: NaiveDate },
}

#[derive(Clone, Copy, Debug)]
pub enum UpdateErr {
    ProxyPing,

    FetchUrl { date: NaiveDate },
    // FetchImage { date: NaiveDate },
    // SaveImage { date: NaiveDate },
}

impl State {
    pub fn new(total_units: usize) -> Self {
        Self {
            status: Status::PingProxy,
            latest_update: None,
            is_first_draw: true,

            completed_units: 0,
            total_units,
        }
    }

    pub fn advance_status(&mut self) {
        match self.status {
            Status::PingProxy => self.status = Status::Working,
            Status::Working => self.status = Status::Epilogue,
            Status::Epilogue => (),
            Status::Failed => (),
        }
    }

    pub fn update(&mut self, update: UpdateResult) {
        self.latest_update = Some(update);

        match update {
            Err(_) => {
                self.status = Status::Failed;
            }
            Ok(UpdateOk::SaveImage { .. }) => {
                self.increase_complete_units();
            }
            _ => (),
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

    pub fn completed_units(&self) -> usize {
        self.completed_units
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn latest_update(&self) -> Option<UpdateResult> {
        self.latest_update
    }

    pub fn total_units(&self) -> usize {
        self.total_units
    }

    pub fn is_failed(&self) -> bool {
        self.status == Status::Failed
    }

    pub fn record_draw(&mut self) -> bool {
        let was_first_draw = self.is_first_draw;
        self.is_first_draw = false;
        was_first_draw
    }
}
