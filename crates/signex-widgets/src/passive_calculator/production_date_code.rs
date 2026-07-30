use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProductionDateCycle {
    TwentyYear,
    TenYear,
    FourYear,
}

impl ProductionDateCycle {
    pub const ALL: [Self; 3] = [Self::TwentyYear, Self::TenYear, Self::FourYear];
}

impl fmt::Display for ProductionDateCycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TwentyYear => "20-year cycle",
            Self::TenYear => "10-year cycle",
            Self::FourYear => "4-year cycle",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProductionMonth {
    January = 1,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl ProductionMonth {
    pub const ALL: [Self; 12] = [
        Self::January,
        Self::February,
        Self::March,
        Self::April,
        Self::May,
        Self::June,
        Self::July,
        Self::August,
        Self::September,
        Self::October,
        Self::November,
        Self::December,
    ];

    const fn index(self) -> usize {
        self as usize - 1
    }
}

impl fmt::Display for ProductionMonth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::January => "January",
            Self::February => "February",
            Self::March => "March",
            Self::April => "April",
            Self::May => "May",
            Self::June => "June",
            Self::July => "July",
            Self::August => "August",
            Self::September => "September",
            Self::October => "October",
            Self::November => "November",
            Self::December => "December",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProductionDateCode {
    TwentyYear { year: u16, month: ProductionMonth },
    TenYear { year: u16, month: ProductionMonth },
    FourYear { year: u16, month: ProductionMonth },
}

impl ProductionDateCode {
    pub const fn new(cycle: ProductionDateCycle, year: u16, month: ProductionMonth) -> Self {
        match cycle {
            ProductionDateCycle::TwentyYear => Self::TwentyYear { year, month },
            ProductionDateCycle::TenYear => Self::TenYear { year, month },
            ProductionDateCycle::FourYear => Self::FourYear { year, month },
        }
    }

    pub const fn cycle(self) -> ProductionDateCycle {
        match self {
            Self::TwentyYear { .. } => ProductionDateCycle::TwentyYear,
            Self::TenYear { .. } => ProductionDateCycle::TenYear,
            Self::FourYear { .. } => ProductionDateCycle::FourYear,
        }
    }

    pub const fn year(self) -> u16 {
        match self {
            Self::TwentyYear { year, .. }
            | Self::TenYear { year, .. }
            | Self::FourYear { year, .. } => year,
        }
    }

    pub const fn month(self) -> ProductionMonth {
        match self {
            Self::TwentyYear { month, .. }
            | Self::TenYear { month, .. }
            | Self::FourYear { month, .. } => month,
        }
    }
}

impl fmt::Display for ProductionDateCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match *self {
            Self::TwentyYear { year, month } => {
                format!(
                    "{}{}",
                    twenty_year_code(year),
                    twenty_year_month_code(month)
                )
            }
            Self::TenYear { year, month } => {
                format!("{}{}", year % 10, ten_year_month_code(month))
            }
            Self::FourYear { year, month } => four_year_code(year, month).to_string(),
        };
        formatter.write_str(&code)
    }
}

const fn twenty_year_code(year: u16) -> char {
    const CODES: [char; 20] = [
        'M', 'N', 'P', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'A', 'B', 'C', 'D', 'E', 'F', 'H', 'J',
        'K', 'L',
    ];
    CODES[(year % 20) as usize]
}

const fn twenty_year_month_code(month: ProductionMonth) -> char {
    match month {
        ProductionMonth::January => '1',
        ProductionMonth::February => '2',
        ProductionMonth::March => '3',
        ProductionMonth::April => '4',
        ProductionMonth::May => '5',
        ProductionMonth::June => '6',
        ProductionMonth::July => '7',
        ProductionMonth::August => '8',
        ProductionMonth::September => '9',
        ProductionMonth::October => 'O',
        ProductionMonth::November => 'N',
        ProductionMonth::December => 'D',
    }
}

const fn ten_year_month_code(month: ProductionMonth) -> char {
    match month {
        ProductionMonth::January => '1',
        ProductionMonth::February => '2',
        ProductionMonth::March => '3',
        ProductionMonth::April => '4',
        ProductionMonth::May => '5',
        ProductionMonth::June => '6',
        ProductionMonth::July => '7',
        ProductionMonth::August => '8',
        ProductionMonth::September => '9',
        ProductionMonth::October => 'X',
        ProductionMonth::November => 'Y',
        ProductionMonth::December => 'Z',
    }
}

const fn four_year_code(year: u16, month: ProductionMonth) -> char {
    const CODES: [[char; 12]; 4] = [
        ['n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'],
        ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'J', 'K', 'L', 'M'],
        ['N', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z'],
        ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'j', 'k', 'l', 'm'],
    ];
    CODES[(year % 4) as usize][month.index()]
}
