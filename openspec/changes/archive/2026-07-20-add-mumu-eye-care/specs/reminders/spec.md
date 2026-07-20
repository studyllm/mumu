## ADDED Requirements

### Requirement: Default reminder parameters

The system SHALL use the following defaults for reminders: work hours 09:00-18:00, interval 20 minutes, rest duration 20 seconds, calculation starts from the last completed reminder.

#### Scenario: First reminder after launch

- **WHEN** software launches at 09:00
- **THEN** the first reminder SHALL trigger 20 minutes after launch

#### Scenario: Subsequent reminder after completion

- **WHEN** user completes (or skips) a 20-second rest
- **THEN** the next reminder SHALL be scheduled 20 minutes after the rest completed

### Requirement: Configurable reminder parameters

The system SHALL allow users to configure work hours, interval (15-60 minutes, step 5), and rest duration (10-60 seconds, step 5) via the settings page.

#### Scenario: User changes interval

- **WHEN** user moves the interval slider to 30 minutes
- **THEN** the next reminder SHALL be scheduled 30 minutes after the previous completion

#### Scenario: User changes rest duration

- **WHEN** user moves the rest slider to 30 seconds
- **THEN** the next reminder popup SHALL show a 30-second countdown

#### Scenario: Invalid work hours

- **WHEN** user sets end time earlier than or equal to start time
- **THEN** the system SHALL display a validation error and reject the change

### Requirement: Reminder trigger conditions

The system SHALL trigger a reminder only when ALL of the following are true: current time is within configured work hours, time since last completed reminder is at least the configured interval, user is not in a paused state, screen is on, no fullscreen application is active, and user is not locked.

#### Scenario: All conditions met

- **WHEN** current time is 10:30 (within 09:00-18:00), 20+ minutes since last rest, screen is on, no fullscreen app, user not paused
- **THEN** the reminder popup SHALL appear at the bottom-right of the primary monitor

#### Scenario: Outside work hours

- **WHEN** current time is 19:00 (outside work hours)
- **THEN** the reminder SHALL NOT trigger regardless of other conditions

#### Scenario: User paused

- **WHEN** user selected "Pause for 30 minutes" 10 minutes ago
- **THEN** the reminder SHALL NOT trigger until the pause expires

### Requirement: Reminder popup lifecycle

The system SHALL display the reminder popup at the bottom-right of the primary monitor (24px from edges, 320×200px), with a 300ms fade-in, countdown display, and 500ms fade-out when countdown reaches zero.

#### Scenario: Popup appears

- **WHEN** trigger conditions are met
- **THEN** the popup SHALL fade in within 300ms at the bottom-right corner

#### Scenario: Countdown completes

- **WHEN** countdown reaches 0
- **THEN** the popup SHALL play the wooden fish sound (if enabled) and fade out within 500ms

#### Scenario: User skips

- **WHEN** user clicks the skip button
- **THEN** the popup SHALL close immediately without playing the wooden fish sound

### Requirement: Countdown display

The system SHALL display the remaining rest time as a 64px bold number in the popup center, updating every second.

#### Scenario: Countdown updates

- **WHEN** 5 seconds have elapsed since popup appeared with 20-second rest
- **THEN** the displayed number SHALL be 15

### Requirement: Fullscreen application handling

The system SHALL NOT display reminder popups when any application is running in fullscreen mode, and SHALL NOT replay missed reminders after the fullscreen application exits.

#### Scenario: Fullscreen app active

- **WHEN** user is presenting in fullscreen PowerPoint
- **THEN** the reminder popup SHALL NOT appear

#### Scenario: Fullscreen app exits

- **WHEN** user exits the fullscreen application
- **THEN** the next reminder SHALL trigger 20 minutes after the last completed reminder (not after exit)

### Requirement: Lock screen handling

The system SHALL hide any visible reminder popup immediately when the screen locks, pause the countdown, and resume the countdown from the paused position when the screen unlocks (if still within work hours).

#### Scenario: Lock during popup

- **WHEN** user locks the screen 10 seconds into a 20-second rest
- **THEN** the popup SHALL hide and the countdown SHALL pause

#### Scenario: Unlock within work hours

- **WHEN** user unlocks the screen 5 minutes later at 10:00
- **THEN** the popup SHALL reappear with 10 seconds remaining on the countdown

#### Scenario: Lock outside work hours

- **WHEN** user locks the screen at 17:55 and unlocks at 18:30 (outside work hours)
- **THEN** the reminder SHALL NOT reappear

### Requirement: Shutdown and hibernation handling

The system SHALL NOT replay reminders that were missed due to system shutdown or hibernation after the system restarts.

#### Scenario: System shuts down during rest

- **WHEN** user shuts down the computer 8 seconds into a 20-second rest
- **THEN** on next boot, no popup SHALL reappear for the missed rest

#### Scenario: System hibernates

- **WHEN** system hibernates during a work period
- **THEN** on resume, the next reminder SHALL be scheduled 20 minutes from the last completed rest (not from resume time)

### Requirement: Reminder pause

The system SHALL allow users to pause reminders via tray menu with three options: 30 minutes, 1 hour, or until tomorrow 09:00.

#### Scenario: User pauses for 30 minutes

- **WHEN** user selects "Pause for 30 minutes" from tray menu
- **THEN** reminders SHALL NOT trigger for 30 minutes, after which normal scheduling resumes

#### Scenario: User pauses until tomorrow

- **WHEN** user selects "Pause until tomorrow 09:00" at 14:00 on Monday
- **THEN** reminders SHALL NOT trigger until 09:00 on Tuesday

### Requirement: Rest count recording

The system SHALL record each completed reminder (countdown reached zero OR user skipped) as a rest event in the daily stats, incrementing the rest count and adding the configured rest duration to the rest seconds.

#### Scenario: User completes rest

- **WHEN** countdown reaches zero
- **THEN** `rest_count` SHALL increment by 1 and `rest_seconds` SHALL increase by the configured rest duration

#### Scenario: User skips rest

- **WHEN** user clicks the skip button
- **THEN** `rest_count` SHALL still increment by 1 (skip counts as rest) and `rest_seconds` SHALL increase by elapsed time

### Requirement: Cross-period reminders

The system SHALL silently trigger (popup visible, no sound) if the scheduled reminder time falls within 20 minutes of the work hours end.

#### Scenario: Late work hours reminder

- **WHEN** configured end time is 18:00 and reminder is scheduled for 17:55
- **THEN** the popup SHALL appear normally but the wooden fish sound SHALL NOT play

### Requirement: Eye drop and warm compress reminders (soft prompts)

The system SHALL provide soft prompts (separate from the 20-20-20 strong reminder) to remind the user to apply eye drops and use a warm compress. These prompts SHALL appear as smaller, non-modal notifications that auto-dismiss after 10 seconds and SHALL NOT include a countdown or force any user action.

#### Scenario: Eye drop reminder default

- **WHEN** 2 hours have elapsed since the last eye drop reminder (or since software start), and current time is within work hours
- **THEN** a soft prompt SHALL appear at the bottom-right with the message "该滴眼药水了" and auto-dismiss after 10 seconds

#### Scenario: Warm compress reminder default

- **WHEN** current time reaches 13:00 (or the user-configured warm compress time) on a workday
- **THEN** a soft prompt SHALL appear with the message "该热敷眼罩了" and auto-dismiss after 10 seconds

#### Scenario: User dismisses eye drop prompt

- **WHEN** user clicks the dismiss button on an eye drop prompt
- **THEN** the prompt SHALL close immediately and the next eye drop reminder SHALL be scheduled 30 minutes later

#### Scenario: User dismisses warm compress prompt

- **WHEN** user clicks the dismiss button on a warm compress prompt
- **THEN** the prompt SHALL close and the next warm compress reminder SHALL be scheduled 1 hour later

#### Scenario: Repeated eye drop dismissals

- **WHEN** user dismisses the eye drop prompt 3 consecutive times
- **THEN** no further eye drop reminders SHALL appear until the next workday to avoid nuisance

#### Scenario: Soft prompt distinguishes from strong reminder

- **WHEN** a soft prompt (eye drop or warm compress) is displayed
- **THEN** the prompt SHALL use a smaller UI (280×80px), no countdown number, and SHALL NOT play the wooden fish sound

#### Scenario: Eye drop reminders disabled

- **WHEN** user disables eye drop reminders in settings
- **THEN** no eye drop prompts SHALL appear and no statistics SHALL be recorded for them

#### Scenario: Warm compress reminders disabled

- **WHEN** user disables warm compress reminders in settings
- **THEN** no warm compress prompts SHALL appear at the configured time

### Requirement: Wooden fish sound

The system SHALL play a 200ms wooden fish sound when the countdown reaches zero, controlled by the "Play sound on completion" setting (default: enabled).

#### Scenario: Sound enabled

- **WHEN** countdown reaches zero and "Play sound on completion" is enabled
- **THEN** the wooden fish sound SHALL play at the system volume

#### Scenario: Sound disabled

- **WHEN** countdown reaches zero and "Play sound on completion" is disabled
- **THEN** no sound SHALL play

#### Scenario: System muted

- **WHEN** system volume is muted
- **THEN** the wooden fish sound SHALL NOT play regardless of setting
