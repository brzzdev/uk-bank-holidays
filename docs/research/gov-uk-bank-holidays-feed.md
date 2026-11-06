# The gov.uk bank holidays feed in practice

- Status: Accepted
- Date: 2026-11-06
- Ticket: [#4](https://github.com/brzzdev/uk-bank-holidays/issues/4)

## Context

The app and the CLI share a core that reads https://www.gov.uk/bank-holidays.json, caches it on
disk and refreshes it when stale. We need to know what the feed contains, how far ahead it reaches,
how it marks substitute days and one-off holidays, how it caches over HTTP, and what terms apply.

Sources: the live feed fetched on 2026-11-06 with a generic User-Agent, the code that serves it in
[alphagov/frontend](https://github.com/alphagov/frontend) (`app/controllers/calendar_controller.rb`,
`app/models/calendar.rb`, `app/models/calendar/division.rb`, `lib/data/bank-holidays.json`,
`docs/calendars.md`) and that file's commit history, the
[API Catalogue entry](https://www.api.gov.uk/gds/bank-holidays/) and the
[GOV.UK Content API docs](https://content-api.publishing.service.gov.uk/).

## Findings

### Shape

One JSON object keyed by division slug, always in this order: `england-and-wales`, `scotland`,
`northern-ireland`. Each value repeats its slug in `division` and holds `events`, sorted by date
ascending. Every event has exactly four keys, all always present:

| Key       | Type   | Meaning                                                              |
| --------- | ------ | -------------------------------------------------------------------- |
| `title`   | string | English display name, with typographic apostrophes (`New Year’s Day`) |
| `date`    | string | ISO `YYYY-MM-DD`, the day the holiday is observed                     |
| `notes`   | string | `""` or `"Substitute day"`; nothing else appears in the current data  |
| `bunting` | bool   | Whether gov.uk hangs bunting on the day; `false` for sombre days      |

Trimmed sample:

```json
{
  "england-and-wales": {
    "division": "england-and-wales",
    "events": [
      {"title": "Good Friday", "date": "2026-04-03", "notes": "", "bunting": false},
      {"title": "Boxing Day", "date": "2026-12-28", "notes": "Substitute day", "bunting": true}
    ]
  },
  "scotland": {
    "division": "scotland",
    "events": [
      {"title": "World Cup bank holiday", "date": "2026-06-15", "notes": "", "bunting": true}
    ]
  },
  "northern-ireland": {
    "division": "northern-ireland",
    "events": [
      {"title": "Battle of the Boyne (Orangemen’s Day)", "date": "2026-07-13", "notes": "Substitute day", "bunting": false}
    ]
  }
}
```

The JSON is rendered from `lib/data/bank-holidays.json`, where titles and notes are i18n keys
(`bank_holidays.new_year`, `common.substitute_day`) and dates are `DD/MM/YYYY`; `Calendar#as_json`
translates and reformats them. The source file carries a `last_updated` date (`24/11/2025` today),
but the feed does not expose it.

Per-division feeds also exist at `/bank-holidays/{division}.json` (shape `{"division", "events"}`,
about 6.5 KB) and `.ics`, all linked from https://www.gov.uk/bank-holidays.

### Coverage window and when years appear

The live feed covers 2019-01-01 to 2028-12-26: 83 events for England and Wales, 94 for Scotland,
103 for Northern Ireland. Years are added by hand-edited pull requests to `lib/data/bank-holidays.json`
about two years ahead, usually in November:

| Commit date | Change                                    |
| ----------- | ----------------------------------------- |
| 2021-12-03  | Add generated dates for 2023              |
| 2022-11-10  | Add dates for 2024 and 2025; coronation   |
| 2023-12-15  | Bank holidays changes 2026                |
| 2024-11-27  | Bank holidays, clock changes update 2027  |
| 2025-11-24  | Add 2028 bank holiday dates               |
| 2026-02-25  | Add World Cup bank holiday for Scotland   |

So at any moment the feed reaches the end of the year after next, and a 2029 update is due around
now. The rake task that once generated dates is gone (`lib/tasks` holds none); edits are manual.

The start of the window moves too. Old years were pruned in 2022 ("Remove 2017 bank holiday
dates"), and in August 2025 commit `7367e82e4` removed 2019 to 2023 because the feed "is responsible
for about 6% of GOV.UK traffic" and "I don't think we have a documented commitment to retain old
dates indefinitely". That was reverted in January 2026, but the intent stands: history is not
guaranteed.

### Substitute days and one-off holidays

- A substitute day is an ordinary event with the original holiday's title, the observed date, and
  `notes: "Substitute day"` (e.g. `Boxing Day` on 2026-12-28). The date the holiday "really" falls
  on is not listed when it lands on a weekend. In Scotland, `2nd January` and `St Andrew’s Day` are
  substituted the same way.
- One-off holidays are plain events with a bespoke title and empty `notes`: `Platinum Jubilee bank
  holiday` (2022-06-03), `Bank Holiday for the State Funeral of Queen Elizabeth II` (2022-09-19,
  `bunting: false`), `Bank holiday for the coronation of King Charles III` (2023-05-08), `World Cup
  bank holiday` (Scotland only, 2026-06-15). Older data used `notes: "Extra bank holiday"`
  (still in `spec/system/json_spec.rb`), so a parser should accept any `notes` string.
- A moved holiday replaces the regular one rather than being flagged: 2020's early May holiday is a
  single event, `Early May bank holiday (VE day)` on 2020-05-08, with no event on the Monday.
- Divisions differ: Scotland has `2nd January` and `St Andrew’s Day` but no `Easter Monday`;
  Northern Ireland adds `St Patrick’s Day` and the Battle of the Boyne. One-offs may apply to one
  division only.
- `bunting` is a presentation flag: false for every Good Friday, the State Funeral, every Battle of
  the Boyne, and Easter Monday 2020 (lockdown). It is not a type marker.

There is no field that identifies a holiday kind; the only stable identity is division plus date.

### HTTP behaviour

Headers on 2026-11-06, served by Fastly over nginx:

- `cache-control: max-age=3600, public` for the combined feed (set by `set_expiry 1.hour` in
  `calendar_controller.rb`); the per-division feeds get `max-age=86400`.
- `etag: W/"538b3482c28b85ecd2db606a0d5ae6ad"`, a weak ETag. `If-None-Match` with it returns
  `304` and an empty body.
- No `Last-Modified` header. `If-Modified-Since` nonetheless returned `304` from the CDN, so do not
  rely on it; use the ETag.
- `access-control-allow-origin: *`, `content-type: application/json; charset=utf-8`.
- Size: 22,207 bytes uncompressed, 1,338 bytes with `Accept-Encoding: gzip`.
- `age` and `x-cache: HIT` show responses come from the CDN, so a fetch is cheap for both sides.

### Terms and rate limits

- The API Catalogue lists it as GDS's "Bank Holidays" API, documented only by `docs/calendars.md`
  in alphagov/frontend. No key, no registration, no SLA, no versioning.
- gov.uk content, this feed included, is under the
  [Open Government Licence v3.0](https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/):
  free reuse with attribution ("Contains public sector information licensed under the Open
  Government Licence v3.0").
- No rate limit is documented for the feed itself. The nearest stated policy, for the GOV.UK
  Content API, is 10 requests per second per client.

## Implications

- Cache the raw response bytes plus the ETag and fetch time. Treat the cache as fresh for at least
  the server's `max-age` (an hour); data changes a few times a year, so a much longer refresh
  interval (a day or more) is reasonable, with a conditional `If-None-Match` request that costs a
  304 when nothing changed. Ask for gzip.
- Model an event as `{division, date, title, notes, bunting}` and key it by division and date.
  Accept unknown `notes` values and titles; never match on titles to infer a holiday kind.
- "Is this a substitute day" is `notes == "Substitute day"`; one-off and moved holidays are
  indistinguishable from regular ones in the data, which is fine for display.
- The feed ends at the end of the year after next and the next year arrives around late November,
  so predicted bank holidays only need to fill the years beyond the feed's last date, per division,
  and must yield to real data once it appears. Since one-offs and moves are announced ad hoc,
  predictions are always "usual pattern" guesses.
- Do not assume old years stay in the feed. Anything that shows past holidays should either accept
  a window starting at last year or keep previously seen years in the cache.
- Show the OGL attribution somewhere in the app (an About panel) and the CLI (`--version` or help).
