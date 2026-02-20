---
name: parser-dev
description: Develop and debug release title parsing regex patterns
---

# Release Title Parser Development

You are working on the release title parser in pir9 — the component that extracts structured metadata from torrent/NZB release titles like `Show.Name.S01E05.720p.HDTV.x264-GROUP`.

## Key Files

- `src/core/parser/mod.rs` — Main parser with regex patterns and `ParsedEpisodeInfo`
- `src/core/parser/series.rs` — Series-specific parsing, `parse_quality_from_filename()`, `parse_release_group()`
- `src/core/parser/quality.rs` — Quality tier definitions (SDTV → Bluray remux)

## Important Patterns

### Regex definitions
All patterns use `once_cell::sync::Lazy<Regex>` for compile-once semantics:
- `SEASON_EPISODE_REGEX` — `S01E05` format
- `ALT_SEASON_EPISODE_REGEX` — `1x05` format
- `FULL_SEASON_REGEX` — `S01` without episode
- `DAILY_REGEX` — `2024.01.15` date-based episodes
- `ABSOLUTE_EPISODE_REGEX` — `- 05` anime absolute numbering
- `YEAR_REGEX` — Year extraction (note: needs `|$` to match years at end-of-string)
- `RELEASE_GROUP_REGEX` — `-GROUP` at end of title

### Known gotchas
1. **`YEAR_REGEX` must include `|$`** — cleaned titles often end with the year and no trailing separator
2. **`best_series_match()`** scores candidates by title + year proximity — it replaces boolean `.find()` at all match sites
3. **Quality tiers**: SDTV < DVD < WEBDL480 < HDTV720 < WEBDL1080 < Bluray1080 < Bluray2160 (remux variants higher)
4. **Anime**: May use absolute episode numbers without season, double-check `ABSOLUTE_EPISODE_REGEX` handling

## Testing Approach

Always write unit tests with realistic release titles:
```rust
#[test]
fn test_parse_standard_release() {
    let result = parse_title("Show.Name.S01E05.720p.HDTV.x264-GROUP");
    assert_eq!(result.series_title, "Show Name");
    assert_eq!(result.season_number, Some(1));
    assert_eq!(result.episode_numbers, vec![5]);
    assert_eq!(result.quality, Quality::HDTV720p);
    assert_eq!(result.release_group, Some("GROUP".to_string()));
}
```

Test edge cases:
- Multi-episode: `S01E05E06`, `S01E05-E08`
- Daily shows: `Show.Name.2024.01.15`
- Anime: `[SubGroup] Show Name - 05 [1080p]`
- Year in title: `Show.Name.2024.S01E05`
- No quality info: `Show.Name.S01E05-GROUP`
- Special characters in titles

## Workflow
1. Read the current parser code to understand existing patterns
2. Add or modify regex patterns as needed
3. Write tests FIRST with expected parsing results
4. Implement the parsing logic
5. Run `cargo test` to verify all tests pass (including existing ones)
