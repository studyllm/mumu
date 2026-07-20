## ADDED Requirements

### Requirement: Effective usage calculation

The system SHALL calculate effective daily usage as: `screen_on_duration − locked_duration − idle_duration_30min`. Idle duration is the time when neither keyboard nor mouse input was detected for 30+ minutes.

#### Scenario: Normal usage

- **WHEN** screen was on for 8 hours with no lock or idle periods
- **THEN** effective usage SHALL be 8 hours

#### Scenario: Lock excluded

- **WHEN** screen was on for 8 hours but locked for 1 hour
- **THEN** effective usage SHALL be 7 hours

#### Scenario: Idle excluded

- **WHEN** screen was on for 8 hours with no lock but idle for 30 minutes
- **THEN** effective usage SHALL be 7 hours 30 minutes

### Requirement: Fullscreen apps count as active

The system SHALL NOT mark a session as idle when a fullscreen application is active, regardless of keyboard/mouse input.

#### Scenario: Video conference

- **WHEN** user is in a 1-hour video conference with no keyboard input
- **THEN** the entire hour SHALL count as effective usage (not idle)

#### Scenario: Watching video

- **WHEN** user is watching a fullscreen video with no input for 45 minutes
- **THEN** the 45 minutes SHALL count as effective usage

### Requirement: 30-second sampling interval

The system SHALL sample screen state every 30 seconds to balance accuracy and CPU consumption.

#### Scenario: Regular sampling

- **WHEN** the statistics module is running
- **THEN** it SHALL poll screen status every 30 seconds via a background timer

### Requirement: Daily stats persistence

The system SHALL persist daily stats in SQLite at `%APPDATA%\沐目\stats.db` with the schema: `daily_stats(date PRIMARY KEY, total_seconds, rest_count, rest_seconds, created_at, updated_at)`.

#### Scenario: Daily rollup

- **WHEN** a new day starts (00:00 local time)
- **THEN** the previous day's accumulated seconds SHALL be written to `daily_stats` and a new row SHALL begin

#### Scenario: First launch

- **WHEN** software launches for the first time
- **THEN** the SQLite database SHALL be created with the schema and today's row initialized to zero

### Requirement: Pause records

The system SHALL record all pause events in `pause_records(id, start_time, end_time, reason)` with reason values: `manual`, `lock`, `fullscreen`.

#### Scenario: Manual pause

- **WHEN** user selects "Pause for 30 minutes" from tray menu
- **THEN** a row SHALL be inserted with reason `manual` and the row SHALL be updated with `end_time` when pause expires

#### Scenario: Auto pause on lock

- **WHEN** user locks the screen
- **THEN** a row SHALL be inserted with reason `lock` and `end_time` SHALL be set when screen unlocks

### Requirement: Main window color rules

The system SHALL display the main usage number in default color for ≤8h, default color with warning text for 8-10h, and default color with warning border + suggestion text for >10h.

#### Scenario: Light usage

- **WHEN** today's usage is 6h 30m
- **THEN** the main number SHALL display in default color with no warning text

#### Scenario: Heavy usage

- **WHEN** today's usage is 9h
- **THEN** the main number SHALL display in default color with a small "注意休息" warning text in warm orange

#### Scenario: Excessive usage

- **WHEN** today's usage is 11h
- **THEN** the main number SHALL display in default color with a warm orange border and a "建议关掉电脑" suggestion text

### Requirement: Data retention on reinstall

The system SHALL retain daily stats when the software is reinstalled (data stored in `%APPDATA%` survives app uninstall/reinstall).

#### Scenario: Reinstall preserves history

- **WHEN** user uninstalls and reinstalls the software
- **THEN** the previous `daily_stats` rows SHALL be available on next launch

### Requirement: Data cleared on uninstall

The system SHALL remove the `%APPDATA%\沐目\` directory completely when the user uninstalls the software via the Windows uninstaller.

#### Scenario: Uninstall removes data

- **WHEN** user uninstalls via Windows Settings → Apps
- **THEN** `%APPDATA%\沐目\` SHALL be deleted along with the application files

### Requirement: No manual data editing

The system SHALL NOT provide any UI or API for users to manually edit, delete, or export their daily stats.

#### Scenario: No edit option

- **WHEN** user views the settings or main window
- **THEN** no "edit stats", "delete data", or "export data" controls SHALL be visible

### Requirement: Performance budget

The system SHALL keep memory usage of the statistics module under 10MB, idle CPU under 0.5%, and disk writes under 1MB/hour.

#### Scenario: Idle system check

- **WHEN** the software is running with no reminders active for 1 hour
- **THEN** the statistics module SHALL consume under 10MB memory and under 0.5% CPU
