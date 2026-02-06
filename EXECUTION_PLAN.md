# Query Studio - Execution Plan for Missing Features

> Generated: January 2026
> Based on gap analysis of IMPLEMENTATION_PLAN.md

---

## Executive Summary

**Current Implementation Status:** ~65-70% Complete

This execution plan prioritizes the remaining features by business value, technical dependencies, and implementation complexity.

---

## Priority Tiers

### 🔴 Tier 1: High Priority (Should complete first)
- Advanced Filtering UI
- Excel Export
- Connection Pooling Optimization

### 🟡 Tier 2: Medium Priority (Important for UX)
- OAuth Integration
- Virtual Scrolling
- Keyboard Shortcuts
- Explanation Caching

### 🟢 Tier 3: Nice-to-Have (Can defer)
- ER Diagram Visualization
- Collaboration/Sharing Features
- Automated AI Insights
- Background Query Execution

---

## Detailed Execution Plan

---

## TIER 1: HIGH PRIORITY

### 1. Advanced Filtering UI (FilterPanel.vue)

**Why:** Users currently cannot filter query results without writing new SQL. This is a core UX gap.

**Effort:** 3-4 days

**Files to create:**
- `src/components/FilterPanel.vue`

**Files to modify:**
- `src/ResultsTable.vue`
- `src/Home.vue`

**Implementation Steps:**

```
Day 1: Component Structure
├── Create FilterPanel.vue with basic UI
├── Add filter state management
└── Implement filter types (text, numeric, date, boolean)

Day 2: Filter Logic
├── Text filters: contains, equals, starts with, ends with
├── Numeric filters: equals, greater than, less than, between
├── Date filters: before, after, between
└── Boolean filters: true/false/null

Day 3: Integration
├── Connect FilterPanel to ResultsTable
├── Implement client-side filtering
├── Add filter persistence (localStorage)
└── Add filter presets/saved filters

Day 4: Polish
├── Add filter chips/badges showing active filters
├── Clear all filters button
├── Filter count indicator
└── Testing and edge cases
```

**Component API:**
```typescript
interface Filter {
  id: string;
  column: string;
  operator: 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte' | 'contains' | 'startsWith' | 'endsWith' | 'between' | 'isNull' | 'isNotNull';
  value: string | number | [number, number];
  enabled: boolean;
}

interface FilterPreset {
  id: string;
  name: string;
  filters: Filter[];
}
```

---

### 2. Excel (.xlsx) Export

**Why:** CSV/JSON is insufficient for business users who need Excel-formatted reports.

**Effort:** 2 days

**Dependencies:** Add `rust_xlsxwriter` crate

**Files to modify:**
- `src-tauri/Cargo.toml`
- `src-tauri/src/export.rs`
- `src/components/ExportModal.vue`

**Implementation Steps:**

```
Day 1: Backend
├── Add rust_xlsxwriter to Cargo.toml
├── Implement export_to_xlsx function in export.rs
├── Handle data type formatting (dates, numbers, strings)
├── Add basic styling (headers, column widths)
└── Return as base64 or write to temp file

Day 2: Frontend
├── Add xlsx option to ExportModal.vue
├── Handle file download for xlsx format
├── Add xlsx-specific options (sheet name, auto-width)
└── Testing with various data types
```

**Rust Implementation Outline:**
```rust
// In Cargo.toml
rust_xlsxwriter = "0.64"

// In export.rs
fn export_to_xlsx(
    data: &[HashMap<String, Value>],
    columns: &[String],
    sheet_name: &str,
) -> Result<Vec<u8>, ExportError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    // ... implementation
}
```

---

### 3. Connection Pooling Optimization

**Why:** Current implementation creates new connections per query, causing latency and resource waste.

**Effort:** 2-3 days

**Dependencies:** Refactor database.rs architecture

**Files to modify:**
- `src-tauri/src/database.rs`
- `src-tauri/src/connection_pool.rs`
- `src-tauri/src/lib.rs`

**Implementation Steps:**

```
Day 1: Pool Architecture
├── Create ConnectionPool struct with HashMap<String, Pool>
├── Implement connection key generation (host+port+db+user)
├── Add pool configuration (min/max connections, timeout)
└── Implement get_or_create_pool method

Day 2: Integration
├── Refactor query_db to use pooled connections
├── Add connection health checks
├── Implement connection timeout handling
└── Add pool statistics to cache stats

Day 3: Testing & Cleanup
├── Test with multiple concurrent queries
├── Test connection recovery after errors
├── Add connection disposal on app close
└── Update existing functions to use pool
```

**Pool Structure:**
```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct ConnectionPool {
    postgres_pools: Mutex<HashMap<String, deadpool_postgres::Pool>>,
    mysql_pools: Mutex<HashMap<String, mysql_async::Pool>>,
    max_pool_size: usize,
    connection_timeout: Duration,
}

impl ConnectionPool {
    pub fn new() -> Self { ... }
    pub async fn get_postgres(&self, conn_str: &str) -> Result<...> { ... }
    pub async fn get_mysql(&self, conn_str: &str) -> Result<...> { ... }
    pub fn clear(&self) { ... }
    pub fn get_stats(&self) -> PoolStats { ... }
}
```

---

## TIER 2: MEDIUM PRIORITY

### 4. OAuth Integration (Google + GitHub)

**Why:** Most users expect social login options. Reduces friction.

**Effort:** 5-7 days

**Dependencies:** OAuth libraries, redirect handling in Tauri

**Files to create:**
- `src-tauri/src/oauth.rs`
- `src/components/OAuthCallback.vue`

**Files to modify:**
- `src-tauri/Cargo.toml`
- `src-tauri/src/auth.rs`
- `src-tauri/src/lib.rs`
- `src/components/Login.vue`
- `src/components/Signup.vue`
- `src/router/index.ts`
- `src/stores/auth.ts`

**Implementation Steps:**

```
Day 1-2: Backend OAuth Setup
├── Add oauth2 crate to Cargo.toml
├── Create oauth.rs module
├── Implement Google OAuth flow
│   ├── Generate authorization URL
│   ├── Handle callback with code exchange
│   └── Fetch user info from Google API
├── Implement GitHub OAuth flow
│   ├── Generate authorization URL
│   ├── Handle callback with code exchange
│   └── Fetch user info from GitHub API
└── Create/link OAuth accounts to local users

Day 3-4: Tauri Deep Linking
├── Configure Tauri for custom URL scheme (querystudio://)
├── Handle OAuth redirects in Tauri
├── Pass auth code to backend
└── Implement secure state parameter validation

Day 5: Frontend Integration
├── Update Login.vue OAuth buttons
├── Update Signup.vue OAuth buttons
├── Create OAuthCallback.vue for redirect handling
├── Update auth store with OAuth methods
└── Handle OAuth errors gracefully

Day 6-7: Testing & Polish
├── Test Google OAuth flow end-to-end
├── Test GitHub OAuth flow end-to-end
├── Handle account linking edge cases
├── Add "linked accounts" to settings
└── Security audit
```

**OAuth Flow:**
```
1. User clicks "Sign in with Google"
2. Frontend calls backend: get_oauth_url("google")
3. Backend returns authorization URL with state
4. Tauri opens URL in system browser
5. User authenticates with Google
6. Google redirects to querystudio://oauth/callback?code=...&state=...
7. Tauri intercepts, passes to backend
8. Backend exchanges code for tokens
9. Backend fetches user info
10. Backend creates/links user account
11. Backend returns JWT
12. Frontend stores JWT, redirects to dashboard
```

---

### 5. Virtual Scrolling for Large Datasets

**Why:** Rendering 10,000+ rows crashes browsers. Virtual scrolling only renders visible rows.

**Effort:** 2-3 days

**Dependencies:** Consider using `vue-virtual-scroller` or custom implementation

**Files to modify:**
- `src/ResultsTable.vue`
- `package.json`

**Implementation Steps:**

```
Day 1: Setup
├── Install vue-virtual-scroller (or implement custom)
├── Refactor ResultsTable to use virtual list
├── Calculate row heights dynamically
└── Handle variable height rows

Day 2: Integration
├── Implement scroll position persistence
├── Handle column resizing with virtual scroll
├── Optimize re-renders
└── Add scroll-to-row functionality

Day 3: Polish
├── Handle edge cases (empty data, single row)
├── Add loading indicators while scrolling
├── Test with 100k+ rows
└── Performance profiling
```

---

### 6. Keyboard Shortcuts Configuration

**Why:** Power users expect keyboard shortcuts for common actions.

**Effort:** 2 days

**Files to create:**
- `src/composables/useKeyboardShortcuts.ts`
- `src/components/KeyboardShortcutsModal.vue`

**Files to modify:**
- `src/Home.vue`
- `src/components/SettingsModal.vue`

**Default Shortcuts:**
```typescript
const defaultShortcuts = {
  'executeQuery': { key: 'Enter', modifiers: ['ctrl'] },
  'newQuery': { key: 'n', modifiers: ['ctrl'] },
  'saveSnippet': { key: 's', modifiers: ['ctrl', 'shift'] },
  'openHistory': { key: 'h', modifiers: ['ctrl'] },
  'openSnippets': { key: 'b', modifiers: ['ctrl'] },
  'openSchema': { key: 'e', modifiers: ['ctrl'] },
  'exportData': { key: 'e', modifiers: ['ctrl', 'shift'] },
  'clearQuery': { key: 'l', modifiers: ['ctrl'] },
  'focusQueryInput': { key: 'k', modifiers: ['ctrl'] },
};
```

---

### 7. Query Explanation Caching

**Why:** AI calls are slow and expensive. Caching saves time and API costs.

**Effort:** 1 day

**Files to modify:**
- `src-tauri/src/gemini.rs`

**Implementation:**
```rust
use std::collections::HashMap;
use std::sync::Mutex;

static EXPLANATION_CACHE: Mutex<HashMap<String, (QueryExplanation, Instant)>> = ...;

const CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour

pub async fn explain_query(sql_query: &str) -> Result<QueryExplanation, String> {
    // Check cache first
    if let Some((cached, timestamp)) = get_cached(sql_query) {
        if timestamp.elapsed() < CACHE_TTL {
            return Ok(cached);
        }
    }
    
    // Call API
    let explanation = call_gemini_api(...).await?;
    
    // Store in cache
    set_cached(sql_query, explanation.clone());
    
    Ok(explanation)
}
```

---

## TIER 3: NICE-TO-HAVE

### 8. ER Diagram Visualization

**Why:** Visual representation of table relationships helps understand schema.

**Effort:** 5-7 days

**Dependencies:** D3.js or similar visualization library

**Approach:** Use schema introspection data to build nodes (tables) and edges (foreign keys), then render with force-directed graph.

---

### 9. Collaboration/Sharing Features (Phase 5)

**Why:** Team features are valuable but complex. Defer until core is solid.

**Effort:** 2-3 weeks

**Components:**
- Shared query database
- Permission system
- Share link generation
- Team management UI
- Real-time collaboration (optional)

**Recommendation:** Consider this for v2.0 release.

---

### 10. Automated AI Insights

**Why:** Proactive suggestions improve UX but are complex to implement well.

**Effort:** 1-2 weeks

**Features:**
- Automatic query suggestions based on schema
- Data anomaly detection
- Performance recommendations
- Natural language summaries of results

---

### 11. Background Query Execution

**Why:** Long-running queries block UI. Background execution improves UX.

**Effort:** 3-4 days

**Approach:**
- Run queries in separate Tauri thread
- Add query queue management
- Implement progress/cancellation
- Show notification when complete

---

## Execution Timeline

### Sprint 1 (Week 1-2): Core UX Improvements
```
Week 1:
├── Day 1-4: FilterPanel.vue implementation
└── Day 5: Excel export backend

Week 2:
├── Day 1: Excel export frontend
├── Day 2-4: Connection pooling optimization
└── Day 5: Testing and bug fixes
```

### Sprint 2 (Week 3-4): OAuth & Performance
```
Week 3:
├── Day 1-5: OAuth integration (backend + Tauri setup)

Week 4:
├── Day 1-2: OAuth frontend integration
├── Day 3-4: Virtual scrolling
└── Day 5: Keyboard shortcuts
```

### Sprint 3 (Week 5): Polish & Minor Features
```
├── Day 1: Query explanation caching
├── Day 2-3: Error handling improvements
├── Day 4-5: Testing, documentation, bug fixes
```

### Future Sprints: Tier 3 Features
- ER Diagram Visualization
- Collaboration Features
- AI Insights

---

## Technical Debt to Address

1. **Connection Management:** Create proper connection lifecycle
2. **Error Recovery:** Add retry logic for transient failures
3. **Logging:** Add structured logging throughout
4. **Tests:** Add unit tests for Rust backend
5. **Type Safety:** Improve TypeScript types in frontend

---

## Dependencies to Add

### Rust (Cargo.toml)
```toml
# For Excel export
rust_xlsxwriter = "0.64"

# For OAuth (if not using reqwest directly)
oauth2 = "4.4"

# For better connection pooling
deadpool-postgres = "0.12"
```

### Frontend (package.json)
```json
{
  "dependencies": {
    "vue-virtual-scroller": "^2.0.0-beta.8"
  }
}
```

---

## Success Metrics

| Feature | Success Criteria |
|---------|-----------------|
| Filtering | Users can filter any column type |
| Excel Export | Files open correctly in Excel/Sheets |
| Connection Pool | <100ms connection reuse time |
| OAuth | <30s total auth flow |
| Virtual Scroll | 100k rows render smoothly |
| Keyboard Shortcuts | All actions accessible via keyboard |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| OAuth complexity | Start with Google only, add GitHub later |
| Performance issues | Profile early, set performance budgets |
| Breaking changes | Feature flags for new functionality |
| API rate limits | Implement caching, respect rate limits |

---

## Next Steps

1. **Review this plan** with team/stakeholders
2. **Set up feature branches** for each major item
3. **Start Sprint 1** with FilterPanel.vue
4. **Create GitHub issues** for tracking

---

## Appendix: File Structure After Implementation

```
src/
├── components/
│   ├── FilterPanel.vue          # NEW
│   ├── KeyboardShortcutsModal.vue # NEW
│   ├── OAuthCallback.vue        # NEW
│   └── ... (existing)
├── composables/
│   └── useKeyboardShortcuts.ts  # NEW
└── ... (existing)

src-tauri/src/
├── oauth.rs                     # NEW
├── export.rs                    # MODIFIED (xlsx)
├── connection_pool.rs           # MODIFIED (real pooling)
├── database.rs                  # MODIFIED (use pool)
├── gemini.rs                    # MODIFIED (caching)
└── ... (existing)
```
