use super::{gmtime, localtime, timegm, timelocal};
use super::Error;

/// Safely Manipulate Date and Time
pub struct TmEditor {
    utc: bool,
    t: libc::tm,
}

impl TmEditor {
    /// Create a new instance initialize with epoch 0
    pub fn new(utc: bool) -> Self {
        let t = libc::tm {
            // epoch 0
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 1,
            tm_mon: 0,
            tm_year: 70,
            tm_wday: 4,
            tm_yday: 0,
            tm_isdst: -1,
            tm_gmtoff: -1,
            tm_zone: std::ptr::null(),
        };
        Self { utc, t }
    }

    /// Create a new instance initialize with the specified epoch
    pub fn with_epoch(epoch: i64, utc: bool) -> Result<Self, Error> {
        let t = if utc {
            gmtime(epoch)?
        } else {
            localtime(epoch)?
        };
        Ok(Self { utc, t })
    }

    /// Converts back into Unix epoch
    pub fn into_epoch(mut self) -> Result<i64, Error> {
        let epoch = if self.utc {
            timegm(&mut self.t)?
        } else {
            timelocal(&mut self.t)?
        };
        Ok(epoch)
    }

    /// increases the year by 'years' and resets all smaller fields to their minimum
    pub fn add_years(&mut self, years: libc::c_int) -> Result<(), Error> {
        if years == 0 {
            return Ok(());
        }
        self.t.tm_mon = 0;
        self.t.tm_mday = 1;
        self.t.tm_hour = 0;
        self.t.tm_min = 0;
        self.t.tm_sec = 0;
        self.t.tm_year += years;
        self.normalize_time()
    }

    /// increases the month by 'months' and resets all smaller fields to their minimum
    pub fn add_months(&mut self, months: libc::c_int) -> Result<(), Error> {
        if months == 0 {
            return Ok(());
        }
        self.t.tm_mday = 1;
        self.t.tm_hour = 0;
        self.t.tm_min = 0;
        self.t.tm_sec = 0;
        self.t.tm_mon += months;
        self.normalize_time()
    }

    /// increases the day by 'days' and resets all smaller fields to their minimum
    pub fn add_days(&mut self, days: libc::c_int) -> Result<(), Error> {
        if days == 0 {
            return Ok(());
        }
        self.t.tm_hour = 0;
        self.t.tm_min = 0;
        self.t.tm_sec = 0;
        self.t.tm_mday += days;
        self.normalize_time()
    }

    pub fn year(&self) -> libc::c_int {
        self.t.tm_year + 1900
    } // see man mktime
    pub fn month(&self) -> libc::c_int {
        self.t.tm_mon + 1
    }
    pub fn day(&self) -> libc::c_int {
        self.t.tm_mday
    }
    pub fn hour(&self) -> libc::c_int {
        self.t.tm_hour
    }
    pub fn min(&self) -> libc::c_int {
        self.t.tm_min
    }
    pub fn sec(&self) -> libc::c_int {
        self.t.tm_sec
    }

    // Note: tm_wday (0-6, Sunday = 0) => convert to Sunday = 6
    pub fn day_num(&self) -> libc::c_int {
        (self.t.tm_wday + 6) % 7
    }

    pub fn set_time(
        &mut self,
        hour: libc::c_int,
        min: libc::c_int,
        sec: libc::c_int,
    ) -> Result<(), Error> {
        self.t.tm_hour = hour;
        self.t.tm_min = min;
        self.t.tm_sec = sec;
        self.normalize_time()
    }

    pub fn set_min_sec(&mut self, min: libc::c_int, sec: libc::c_int) -> Result<(), Error> {
        self.t.tm_min = min;
        self.t.tm_sec = sec;
        self.normalize_time()
    }

    fn normalize_time(&mut self) -> Result<(), Error> {
        // libc normalizes it for us
        if self.utc {
            timegm(&mut self.t)?;
        } else {
            timelocal(&mut self.t)?;
        }
        Ok(())
    }

    pub fn set_sec(&mut self, v: libc::c_int) -> Result<(), Error> {
        self.t.tm_sec = v;
        self.normalize_time()
    }

    pub fn set_min(&mut self, v: libc::c_int) -> Result<(), Error> {
        self.t.tm_min = v;
        self.normalize_time()
    }

    pub fn set_hour(&mut self, v: libc::c_int) -> Result<(), Error> {
        self.t.tm_hour = v;
        self.normalize_time()
    }

    pub fn set_mday(&mut self, v: libc::c_int) -> Result<(), Error> {
        self.t.tm_mday = v;
        self.normalize_time()
    }

    pub fn set_mon(&mut self, v: libc::c_int) -> Result<(), Error> {
        self.t.tm_mon = v - 1;
        self.normalize_time()
    }

    pub fn set_year(&mut self, v: libc::c_int) -> Result<(), Error> {
        self.t.tm_year = v - 1900;
        self.normalize_time()
    }
}
