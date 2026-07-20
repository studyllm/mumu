## ADDED Requirements

### Requirement: Tray icon visual states

The system SHALL display a tray icon that visually indicates the current state: open eye when actively tracking work, closed eye during reminder rest period.

#### Scenario: Working state icon

- **WHEN** the software is actively tracking screen usage
- **THEN** the tray icon SHALL display an open eye

#### Scenario: Resting state icon

- **WHEN** a reminder popup is being displayed
- **THEN** the tray icon SHALL display a closed eye

### Requirement: Tray icon interaction

The system SHALL respond to tray icon clicks as follows: left-click does nothing, right-click opens the menu, double-click opens the main window.

#### Scenario: Left click

- **WHEN** user left-clicks the tray icon
- **THEN** no action SHALL occur (consistent with Windows convention)

#### Scenario: Right click

- **WHEN** user right-clicks the tray icon
- **THEN** the context menu SHALL appear

#### Scenario: Double click

- **WHEN** user double-clicks the tray icon
- **THEN** the main window SHALL open

### Requirement: Tray context menu

The system SHALL display a context menu on right-click with the following items: today's usage summary (read-only), Open Main Window, Pause 30 minutes, Pause 1 hour, Pause until tomorrow 09:00, Settings, Quit.

#### Scenario: Menu items present

- **WHEN** user right-clicks the tray icon
- **THEN** the menu SHALL contain all 7 items in the specified order with the summary at the top as read-only text

#### Scenario: User pauses for 30 minutes

- **WHEN** user clicks "Pause 30 minutes" in the menu
- **THEN** reminders SHALL not trigger for 30 minutes and the menu SHALL remain accessible

### Requirement: Main window layout

The system SHALL display the main window (480×360px) with a single vertical column showing: today's usage duration as a 64px bold number at top, "今日屏幕使用" subtitle, then "眼睛休息了 N 次" rest count at bottom.

#### Scenario: Main window displays data

- **WHEN** user double-clicks the tray icon
- **THEN** the main window SHALL open centered (first time) or at remembered position, showing today's usage and rest count

#### Scenario: Close button minimizes to tray

- **WHEN** user clicks the close button on the main window
- **THEN** the window SHALL hide and the software SHALL continue running in the tray

### Requirement: Reminder popup position and size

The system SHALL display the reminder popup (320×200px) at the bottom-right corner of the primary monitor, 24px from each screen edge.

#### Scenario: Popup position

- **WHEN** reminder triggers
- **THEN** the popup SHALL appear at bottom-right with 24px offset from screen edges on the primary monitor

#### Scenario: Multi-monitor

- **WHEN** user has multiple monitors connected
- **THEN** the popup SHALL appear on the primary monitor only

### Requirement: Reminder popup visual style

The system SHALL display the popup with a semi-transparent background with Gaussian blur (20px), 12px rounded corners, soft drop shadow, and system theme-following colors.

#### Scenario: Light mode

- **WHEN** system is in light mode
- **THEN** popup background SHALL be rgba(255,255,255,0.85) with backdrop blur

#### Scenario: Dark mode

- **WHEN** system is in dark mode
- **THEN** popup background SHALL be rgba(31,27,22,0.85) with backdrop blur

### Requirement: Reminder popup animations

The system SHALL animate the popup with a 300ms fade-in on appearance (ease-out) and 500ms fade-out on dismissal (ease-in). The popup SHALL remain static (no breathing/looping animation) while displayed.

#### Scenario: Fade in

- **WHEN** popup appears
- **THEN** opacity SHALL transition from 0 to 1 over 300ms with ease-out curve

#### Scenario: No looping animation

- **WHEN** popup is displayed for 20 seconds
- **THEN** the popup SHALL remain static without scale, position, or opacity changes during display

### Requirement: Skip button

The system SHALL display a skip button in the bottom-right corner of the popup as 11px gray text at 10% opacity, becoming 30% opacity on hover.

#### Scenario: Skip button visibility

- **WHEN** popup is displayed
- **THEN** the skip button SHALL be visible as small gray text "跳过" in the bottom-right

#### Scenario: User skips

- **WHEN** user clicks the skip button
- **THEN** the popup SHALL close immediately and the next reminder SHALL be scheduled 20 minutes from the current time

### Requirement: Popup does not steal focus

The system SHALL display the popup without stealing keyboard focus from the user's current application.

#### Scenario: User is typing

- **WHEN** popup appears while user is typing in another application
- **THEN** keyboard focus SHALL remain in the user's application (no focus theft)

### Requirement: Settings window layout

The system SHALL display the settings window (800×600px) with a single scrolling column containing 4 sections: Reminder Settings, General Settings, Advanced Settings, and a Test Reminder button at the bottom.

#### Scenario: Settings sections

- **WHEN** user opens Settings
- **THEN** the window SHALL display the 4 sections in a vertical scrollable layout without left-side navigation

#### Scenario: Settings window resizable

- **WHEN** user resizes the settings window
- **THEN** the window SHALL resize with minimum dimensions 600×400px

### Requirement: Time range picker

The system SHALL provide two independent time pickers (15-minute step) for work hours start and end.

#### Scenario: User sets work hours

- **WHEN** user selects start time 09:00 and end time 18:00
- **THEN** reminders SHALL only trigger between 09:00 and 18:00

### Requirement: Slider inputs

The system SHALL provide sliders for interval (15-60 minutes, step 5) and rest duration (10-60 seconds, step 5), with the current value displayed in a tooltip above the thumb during drag.

#### Scenario: User adjusts interval slider

- **WHEN** user drags the interval slider to 30
- **THEN** the displayed value SHALL show "30 分钟" above the thumb and the new interval SHALL take effect immediately

### Requirement: Checkbox inputs

The system SHALL provide checkboxes for: show popup (default on), play sound on completion (default on), auto-start at boot (default on, in Advanced), debug mode (default off, in Advanced).

#### Scenario: User disables popup

- **WHEN** user unchecks "Show popup"
- **THEN** reminders SHALL still trigger at the configured interval but no popup SHALL appear (only sound if enabled)

### Requirement: Test reminder button

The system SHALL provide a primary-styled "Test Reminder" button at the bottom of the settings window that triggers a 5-second test popup immediately.

#### Scenario: User clicks Test Reminder

- **WHEN** user clicks the Test Reminder button
- **THEN** a popup SHALL appear immediately with a 5-second countdown without affecting the normal 20-minute scheduling

### Requirement: Window lifecycle

The system SHALL handle window close behaviors as follows: main window close minimizes to tray, settings window close closes the window normally, reminder popup is not closeable by user.

#### Scenario: Main window close

- **WHEN** user clicks the X on the main window
- **THEN** the window SHALL hide and the process SHALL continue running

#### Scenario: Settings window close

- **WHEN** user clicks the X on the settings window
- **THEN** the window SHALL close and the main window SHALL remain in its previous state

### Requirement: Soft prompt (eye drop / warm compress)

The system SHALL display soft prompts for eye drop and warm compress reminders at the bottom-right of the primary monitor (24px from edges), with a smaller size (280×80px), no countdown, and auto-dismiss after 10 seconds.

#### Scenario: Soft prompt appearance

- **WHEN** an eye drop or warm compress reminder triggers
- **THEN** a smaller prompt SHALL appear with a single message line and no countdown number

#### Scenario: Soft prompt auto-dismiss

- **WHEN** a soft prompt has been displayed for 10 seconds without user interaction
- **THEN** the prompt SHALL fade out within 500ms

#### Scenario: Soft prompt dismiss button

- **WHEN** user clicks anywhere on the soft prompt
- **THEN** the prompt SHALL close immediately

#### Scenario: Soft prompt does not block input

- **WHEN** a soft prompt is displayed
- **THEN** the user's keyboard and mouse input SHALL NOT be intercepted by the prompt

### Requirement: System theme follow

The system SHALL follow the system color scheme (light/dark) for all UI windows without providing a manual theme toggle.

#### Scenario: User switches system theme

- **WHEN** user changes Windows system theme from light to dark
- **THEN** all MuMu windows SHALL update their colors to match the new theme
