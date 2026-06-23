## Why

The API details page already has paginated request audit data, but the active page is only local UI state. Users cannot bookmark or share a specific page, and manually editing `/stats` does not jump to the requested request-audit page.

## What Changes

- Add the API details pagination page number to the stats URL.
- Let direct navigation to `/stats?page=<n>` initialize the API details page at that page.
- Let browser back/forward and manual URL changes update the API details page number.
- Keep invalid, missing, or non-positive page values on page 1.
- Do not add deep links for every API details filter in this change.

## Capabilities

### New Capabilities
- `api-detail-url-pagination`: API details request-audit pagination can be represented by and restored from the stats page URL.

### Modified Capabilities

None.

## Impact

- Frontend routing types and helpers in `web/api/types.ts` and `web/shared/browser-route.ts`.
- Stats route handling in `web/app/app-routing.ts` and `web/App.tsx`.
- API details pagination state in `web/features/stats/use-ai-statistics-data.ts`.
- Focused frontend tests for stats URL parsing, pagination URL updates, and URL-driven page changes.
