## ADDED Requirements

### Requirement: Settings file location

The system SHALL persist settings as JSON at `%APPDATA%\沐目\settings.json`.

#### Scenario: First launch

- **WHEN** software launches for the first time
- **THEN** `%APPDATA%\沐目\settings.json` SHALL be created with all default values

#### Scenario: Settings survive restart

- **WHEN** user changes settings and restarts the software
- **THEN** the changes SHALL be reflected after restart

### Requirement: Settings schema

The system SHALL persist settings with the schema: `{ version, reminders: { work_start, work_end, interval_minutes, rest_seconds, show_popup, play_sound }, care: { eye_drop_enabled, eye_drop_interval_minutes, warm_compress_enabled, warm_compress_time }, general: { quick_pause }, advanced: { auto_start, debug_mode } }`.

#### Scenario: All settings fields present

- **WHEN** user opens the settings file
- **THEN** it SHALL contain all top-level keys (version, reminders, care, general, advanced) and all nested fields

### Requirement: Default values

The system SHALL use the following defaults: work_start "09:00", work_end "18:00", interval_minutes 20, rest_seconds 20, show_popup true, play_sound true, eye_drop_enabled true, eye_drop_interval_minutes 120, warm_compress_enabled true, warm_compress_time "13:00", quick_pause "30min", auto_start true, debug_mode false.

#### Scenario: Fresh install defaults

- **WHEN** user installs the software for the first time
- **THEN** all settings SHALL match the documented defaults including eye drop and warm compress reminders being enabled

### Requirement: Version migration

The system SHALL migrate settings files with older `version` values to the current version, preserving user-customized values where possible and applying new defaults only for new fields.

#### Scenario: Missing field migration

- **WHEN** settings file has version 1 and a new field is added in version 2
- **THEN** the missing field SHALL be populated with its default value and `version` updated to 2

#### Scenario: Unknown field preservation

- **WHEN** settings file contains a field not in the current schema
- **THEN** the unknown field SHALL be preserved (not deleted)

### Requirement: Immediate effect

The system SHALL apply all setting changes immediately without requiring a save button.

#### Scenario: User changes interval

- **WHEN** user moves the interval slider from 20 to 30
- **THEN** the next reminder SHALL be scheduled 30 minutes from the last completed rest (no save step needed)

#### Scenario: User toggles auto-start

- **WHEN** user unchecks "Auto-start at boot"
- **THEN** the Windows registry auto-start entry SHALL be removed immediately

### Requirement: Validation

The system SHALL validate that work_end is strictly later than work_start, and reject invalid configurations.

#### Scenario: Invalid time range

- **WHEN** user sets work_end to "09:00" and work_start to "18:00"
- **THEN** the system SHALL display a validation error and not save the change

### Requirement: Slider bounds

The system SHALL clamp slider values to their documented ranges: interval 15-60 minutes, rest 10-60 seconds.

#### Scenario: Out-of-range value

- **WHEN** settings file contains interval_minutes: 5 (below minimum)
- **THEN** the system SHALL treat the value as 15 (clamped to minimum)

### Requirement: Auto-start at boot

The system SHALL write a Windows registry entry at `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` when `auto_start` is true, and remove it when false.

#### Scenario: Auto-start enabled by default

- **WHEN** software is freshly installed
- **THEN** the registry entry SHALL be present (default: enabled)

#### Scenario: User disables auto-start

- **WHEN** user unchecks "Auto-start at boot" in Advanced settings
- **THEN** the registry entry SHALL be removed within 1 second

#### Scenario: System restart verification

- **WHEN** user restarts Windows with auto-start enabled
- **THEN** 沐目 SHALL launch automatically after login

### Requirement: Settings UI hides advanced by default

The system SHALL place auto-start and debug mode toggles under a collapsible "Advanced" section, not in the main settings view.

#### Scenario: Advanced hidden by default

- **WHEN** user opens Settings
- **THEN** the Advanced section SHALL be collapsed and auto-start/debug SHALL not be immediately visible

### Requirement: Care reminder configuration

The system SHALL allow users to configure eye drop reminders (enable/disable, interval 60-240 minutes step 30) and warm compress reminders (enable/disable, fixed time HH:MM in 15-minute steps).

#### Scenario: User adjusts eye drop interval

- **WHEN** user moves the eye drop interval slider to 90 minutes
- **THEN** the next eye drop reminder SHALL be scheduled 90 minutes from the previous one

#### Scenario: User changes warm compress time

- **WHEN** user sets warm compress time to "15:00"
- **THEN** the warm compress reminder SHALL trigger at 15:00 (if within work hours) instead of 13:00

#### Scenario: Warm compress outside work hours

- **WHEN** user sets warm compress time to "20:00" and work hours end at 18:00
- **THEN** the warm compress reminder SHALL NOT trigger (outside work hours)

### Requirement: No export or import

The system SHALL NOT provide any UI or API for users to export or import their settings.

#### Scenario: No export option

- **WHEN** user views the settings window
- **THEN** no "Export", "Import", or "Reset to defaults" buttons SHALL be present
