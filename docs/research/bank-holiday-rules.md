# The rules that produce each division's bank holidays

- Status: accepted
- Date: 2026-11-06
- Ticket: [#5](https://github.com/brzzdev/uk-bank-holidays/issues/5)

## Context

The app shows gov.uk's published bank holidays and, for years the feed doesn't cover yet, predicted
bank holidays for each division (England and Wales, Scotland, Northern Ireland). Prediction needs
rules precise enough to implement, a check that they reproduce the published years, and a record
of how one-off changes have departed from them in the past.

At the time of writing, the live feed (<https://www.gov.uk/bank-holidays.json>) covers 2019–2028.
An archived copy
(<https://web.archive.org/web/2018id_/https://www.gov.uk/bank-holidays.json>) adds 2012–2018.
gov.uk never published data before 2012.

## Findings

### Legal basis

- **Statute.** [Banking and Financial Dealings Act 1971, schedule 1](https://www.legislation.gov.uk/ukpga/1971/80/schedule/1)
  lists each division's statutory days. Every substitution it allows is for a **Sunday** only,
  except St Andrew's Day, which also moves off a Saturday (inserted by the
  [St Andrew's Day Bank Holiday (Scotland) Act 2007, s.1](https://www.legislation.gov.uk/asp/2007/2/section/1)).
- **Proclamation.** [Section 1](https://www.legislation.gov.uk/ukpga/1971/80/section/1) lets the
  monarch move a scheduled day in a given year (s.1(2)) or add a special day (s.1(3)). Every
  remaining day, including all Saturday substitutes, comes from royal proclamations under s.1(3).
  These are made each year, roughly 18 months ahead, published in The Gazette and on the Privy
  Council site. Two examples:
  - [2026-07-08 proclamation](https://privycouncil.independent.gov.uk/wp-content/uploads/2026/07/2026-07-08-Proclamation-England-Wales-and-Northern-Ireland-Bank-Holiday.pdf)
    for 2027: 1 Jan, 3 May and 28 Dec in England, Wales and NI, plus 12 Jul in NI.
  - [2025-06-11 Scotland proclamation](https://privycouncil.independent.gov.uk/wp-content/uploads/2025/06/2025-06-11-Scotland-Bank-Holiday-Proclamation.pdf):
    26 Dec 2025 and 25 May 2026.
- **Common law.** Christmas Day and Good Friday are common-law holidays in England, Wales and NI.
  They aren't 1971 Act bank holidays there (s.1(4) refers to them as a separate category), but
  gov.uk lists them, and the app follows gov.uk.

Where each day comes from:

| Day | England and Wales | Scotland | Northern Ireland |
| --- | --- | --- | --- |
| New Year's Day | proclamation | statute (Sun → 3 Jan) | proclamation |
| 2 January | — | statute (Sun → 3 Jan) | — |
| St Patrick's Day, 17 Mar | — | — | statute (Sun → 18 Mar) |
| Good Friday | common law | statute | common law |
| Easter Monday | statute | — | statute |
| Early May, first Mon in May | proclamation | statute | proclamation |
| Spring, last Mon in May | statute | proclamation | statute |
| Battle of the Boyne, 12 Jul | — | — | proclamation |
| Summer | statute, last Mon in Aug | statute, **first** Mon in Aug | statute, last Mon in Aug |
| St Andrew's Day, 30 Nov | — | statute (Sat/Sun → next Mon) | — |
| Christmas Day | common law | statute (Sun → 26 Dec) | common law |
| Boxing Day | statute (26 Dec unless Sun; 27 Dec if 25 or 26 is Sun) | proclamation | statute, as England and Wales |

Proclamations follow a stable pattern, so in practice gov.uk's dates follow the simple rules
below. When predicting a year, the legal source of a day doesn't matter.

### Easter

Easter Sunday is the Gregorian computus that the
[Calendar (New Style) Act 1750](https://www.legislation.gov.uk/apgb/Geo2/24/23) defines through
its tables. The [Easter Act 1928](https://www.legislation.gov.uk/ukpga/Geo5/18-19/35), which would
fix Easter to the Sunday after the second Saturday in April, is on the statute book but has never
been commenced ("Prospective"). The anonymous Gregorian algorithm (Meeus/Jones/Butcher) gives the
right date in integer arithmetic:

```
a = y % 19;  b = y / 100;  c = y % 100;  d = b / 4;  e = b % 4
f = (b + 8) / 25;  g = (b - f + 1) / 3;  h = (19a + b - d - g + 15) % 30
i = c / 4;  k = c % 4;  l = (32 + 2e + 2i - h - k) % 7;  m = (a + 11h + 22l) / 451
month = (h + l - 7m + 114) / 31;  day = (h + l - 7m + 114) % 31 + 1
```

Good Friday is Easter − 2 days and Easter Monday is Easter + 1. Easter Sunday falls between 22 March
and 25 April, so Good Friday falls between 20 Mar and 23 Apr and Easter Monday between 23 Mar and
26 Apr. Neither can collide with St Patrick's Day (substitute at the latest Mon 19 Mar) or the
early May Monday.

### Rules per division

Definitions used below:

- **Weekend** means Saturday or Sunday.
- **Next Monday** of a date means the date itself if it's a weekday, otherwise the following Monday.

**England and Wales** (8 days):

1. New Year's Day: next Monday of 1 Jan.
2. Good Friday.
3. Easter Monday.
4. Early May bank holiday: first Monday in May.
5. Spring bank holiday: last Monday in May.
6. Summer bank holiday: last Monday in August.
7. Christmas Day and Boxing Day, using the Christmas pair rule below.

**Scotland** (9 days):

1. New Year's Day and "2nd January", using the New Year pair rule below.
2. Good Friday. Scotland has **no Easter Monday**.
3. Early May bank holiday: first Monday in May.
4. Spring bank holiday: last Monday in May.
5. Summer bank holiday: **first** Monday in August.
6. St Andrew's Day: next Monday of 30 Nov.
7. Christmas Day and Boxing Day, using the Christmas pair rule below.

**Northern Ireland** (10 days): the England and Wales days, plus:

- St Patrick's Day: next Monday of 17 Mar.
- Battle of the Boyne (Orangemen's Day): next Monday of 12 Jul.

**Pair rules.** These apply to two consecutive fixed days (25/26 Dec everywhere, and 1/2 Jan in
Scotland). Each day that falls on a weekend moves to the next weekday not already taken. The
possible cases by weekday of the first day:

| First day falls on | Holidays fall on | gov.uk labels |
| --- | --- | --- |
| Mon–Thu | both actual dates | both actual |
| Fri | Fri, then Mon (+3) | second day "Substitute day" |
| Sat | Mon (+2), then Tue (+3) | both "Substitute day" |
| Sun | Mon (+1), then Tue (+2) | see below |

The Sunday case is the only one where gov.uk's labels differ between the two pairs. The dates
are the same either way.

- **Christmas pair:** Mon 26 is labelled Boxing Day (actual) and Tue 27 is Christmas Day
  (substitute). Seen in 2016 and 2022.
- **New Year pair in Scotland:** in 2023 the current feed labels Mon 2 as New Year's Day
  (substitute) and Tue 3 as 2nd January (substitute). The 2018 archive labelled the same case in
  2012 and 2017 the other way round: Mon 2 was 2nd January (actual) and Tue 3 was New Year's Day
  (substitute). gov.uk has been inconsistent here, and the newer form is the better one to copy.

Titles in the feed, for reference: "New Year’s Day", "2nd January", "St Patrick’s Day", "Good
Friday", "Easter Monday", "Early May bank holiday", "Spring bank holiday", "Battle of the Boyne
(Orangemen’s Day)", "Summer bank holiday", "St Andrew’s Day", "Christmas Day", "Boxing Day". They
use curly apostrophes. Substitutes carry `notes: "Substitute day"`.

### Verification against the feed

A throwaway implementation of the rules above was diffed against every published date, 2012–2028,
in all three divisions. Every date matched except the one-offs listed below. The only label
mismatch was Scotland's New Year pair when 1 Jan falls on a Sunday (2012 and 2017, archived feed).
Two other edge cases also check out:

- 1 Jan on a Saturday in Scotland gives Mon 3 and Tue 4 (2022).
- 1 Jan on a Friday in Scotland gives "2nd January" on Mon 4 (2021, 2027).

### One-off deviations

Each of these was made by proclamation and announced at least months ahead. The first group can
be checked in the feed:

| Year | Divisions | Change |
| --- | --- | --- |
| 2012 | all | Spring bank holiday moved from Mon 28 May to Mon 4 Jun. Extra: Tue 5 Jun, Queen's Diamond Jubilee |
| 2020 | all | Early May bank holiday moved from Mon 4 May to Fri 8 May (VE Day 75th anniversary) |
| 2022 | all | Spring bank holiday moved from Mon 30 May to Thu 2 Jun. Extra: Fri 3 Jun, Platinum Jubilee ([Gazette notice 3673202](https://www.thegazette.co.uk/notice/3673202)) |
| 2022 | all | Extra: Mon 19 Sep, State Funeral of Queen Elizabeth II |
| 2023 | all | Extra: Mon 8 May, coronation of King Charles III ([Gazette notice 4203714](https://www.thegazette.co.uk/notice/4203714)) |
| 2026 | Scotland | Extra: Mon 15 Jun, World Cup bank holiday ([proclamation of 2026-02-03](https://privycouncil.independent.gov.uk/wp-content/uploads/2026/02/Scotland-Bank-Holiday-Proclamation.pdf)) |

These earlier ones fall before the feed, and I haven't confirmed them in a primary source: 14 Nov
1973 (wedding of Princess Anne), 7 Jun 1977 (Silver Jubilee), 29 Jul 1981 (wedding of the Prince of
Wales), early May moved to 8 May 1995 (VE Day 50th), 31 Dec 1999 (millennium), spring moved to Tue
4 Jun 2002 with an extra day on Mon 3 Jun (Golden Jubilee), and 29 Apr 2011 (wedding of Prince
William). The early May bank holiday in England, Wales and NI is likewise unconfirmed as starting
in 1978. St Andrew's Day has been a bank holiday since 2007 (asp 2, in force 16 Jan 2007), so
predictions for earlier years would need start dates for each rule. That only matters if the app
ever predicts backwards.

The patterns behind these: a moved day stays within days of its usual date, and an extra day sits
next to a moved day or stands alone. A government can also make a holiday conditional on an event,
as with the motion for 20 July 2026 in England had England won the World Cup, which did not happen.
None of this is predictable from rules.

## Implications

- Predict from the rules above for any year after the feed's last year. The rules alone reproduce
  every published date from 2012 to 2028 apart from the one-offs, so they're a sound basis for
  predicted bank holidays.
- Feed data always beats prediction for the years it covers. The app shouldn't try to model
  one-offs, since every past one-off was announced before it reached the feed.
- A predicted year is at least two years out. Proclamations land about 18 months ahead, and the
  feed already ran to the end of 2028 in November 2026. The app's warning that a predicted bank
  holiday is subject to change is about future one-offs, not about the rules.
- The rules are pure functions of year and division, which fits the repo's separation of decision
  logic from I/O. A natural test is to diff the predictor against a stored copy of the feed and
  allow-list the one-off dates.
- Labels need two decisions: use gov.uk's titles, and copy the 2023 form for Scotland's New Year
  pair on a Sunday. Whether a predicted day keeps its gov.uk title or is marked "predicted" is a
  UI choice outside this note.
- The legal source of a day (statute, proclamation or common law) doesn't affect the dates, so the
  domain model doesn't need it.
