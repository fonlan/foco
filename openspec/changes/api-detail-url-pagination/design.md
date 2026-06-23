## Context

Foco already has a global stats route at `/stats` and the API details request audit already uses server-side `page` and `pageSize` query parameters when calling `GET /api/ai-statistics`. The missing piece is the browser route contract: `BrowserRoute` represents stats as only `{ viewMode: "stats" }`, so the current API details page number is lost on reload, sharing, manual URL edits, and browser history navigation.

## Goals / Non-Goals

**Goals:**
- Represent the API details request-audit page in the stats URL as `/stats?page=<n>`.
- Initialize the API details pagination state from that URL.
- Keep pagination clicks, browser back/forward, and direct URL edits in sync with the loaded audit page.
- Keep invalid or missing page values deterministic by treating them as page 1.

**Non-Goals:**
- No backend API changes; the existing `/api/ai-statistics` pagination parameters are sufficient.
- No URL state for filters, page size, selected request details, column visibility, or API overview inputs.
- No new routing library or dependency.

## Decisions

- Extend the existing stats route instead of adding a new route.
  - `/stats?page=<n>` keeps the current global navigation model and avoids a second URL shape for the same screen.
  - Alternative considered: `/stats/page/<n>`. That adds route parsing branches without improving behavior.

- Store only the page number in `BrowserRoute`.
  - The user asked for the pagination page number, and page size already remains local editable state.
  - Alternative considered: encode all filters in the URL. That is larger work and changes more UX/state contracts than this request needs.

- Parse page values with the existing browser route helpers and clamp invalid values to page 1.
  - Central parsing keeps direct loads, popstate, and route writes consistent.
  - Alternative considered: read `window.location.search` inside the stats panel only. That would bypass the app's existing route flow and make back/forward handling easier to drift.

- Update URL state when the API details page changes.
  - Pagination buttons should call the existing route update path with the new page so the URL and loaded data move together.
  - Filter or page-size changes should keep the existing behavior of resetting to page 1, and the route should reflect `page=1`.

## Risks / Trade-offs

- [History noise] Clicking several pages can add browser history entries. -> Use the existing `pushState` route update behavior so back/forward works as requested; consider `replaceState` only if pagination history becomes noisy in practice.
- [Out-of-range URL page] A bookmarked page can later exceed `totalPages`. -> Let the existing API response/pagination bounds handle the loaded result; subsequent UI pagination can clamp using the returned total pages.
- [Route/state feedback loop] Route writes and state effects can trigger each other. -> Only write the URL when the requested page differs from the current route page, and only update filters when the route page differs from the current filter page.
