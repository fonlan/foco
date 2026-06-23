## 1. Routing Contract

- [x] 1.1 Extend the stats `BrowserRoute` shape to carry a positive `page` number.
- [x] 1.2 Parse `/stats?page=<n>` in `web/shared/browser-route.ts`, treating missing, invalid, or non-positive values as page 1.
- [x] 1.3 Serialize stats routes as `/stats?page=<n>` so opening API details from navigation writes `page=1`.

## 2. API Details Pagination Sync

- [x] 2.1 Pass the active stats route page from app routing into the API details panel.
- [x] 2.2 Update the API details data hook or panel state so route page changes update `filters.page` without a feedback loop.
- [x] 2.3 Update pagination, filter, and page-size handlers so page changes and page resets also update the stats URL.

## 3. Verification

- [x] 3.1 Add focused route helper coverage for stats page parsing, serialization, and invalid page fallback.
- [x] 3.2 Add focused app coverage for direct `/stats?page=2`, pagination button URL updates, and URL-driven page changes.
- [x] 3.3 Run `npm run typecheck -w web` and the targeted stats panel tests.
