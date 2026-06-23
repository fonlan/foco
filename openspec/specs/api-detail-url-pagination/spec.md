## Purpose

Defines the URL-backed pagination contract for the API details request-audit view.

## Requirements

### Requirement: API details page is encoded in the stats URL
The system SHALL include the current API details request-audit page number in the stats URL as a positive integer `page` query parameter.

#### Scenario: Opening API details starts at page 1
- **WHEN** the user opens the API details view without a specific page request
- **THEN** the browser URL is `/stats?page=1`
- **AND** the request audit loads page 1

#### Scenario: Pagination updates the URL
- **WHEN** the user selects request-audit page 2 from the API details pagination controls
- **THEN** the browser URL is `/stats?page=2`
- **AND** the request audit loads page 2

### Requirement: Stats URL page drives API details pagination
The system SHALL initialize and update the API details request-audit pagination from the `page` query parameter on the stats URL.

#### Scenario: Direct URL opens a specific page
- **WHEN** the user navigates directly to `/stats?page=3`
- **THEN** the API details request audit loads page 3
- **AND** the active pagination state indicates page 3

#### Scenario: Browser navigation changes the page
- **WHEN** browser navigation changes the current stats URL from `/stats?page=1` to `/stats?page=4`
- **THEN** the API details request audit updates to page 4
- **AND** the next API statistics request uses `page=4`

#### Scenario: Invalid URL page falls back to page 1
- **WHEN** the user navigates to `/stats?page=0`, `/stats?page=-1`, `/stats?page=abc`, or `/stats` without a page value
- **THEN** the API details request audit loads page 1
- **AND** the active pagination state indicates page 1
