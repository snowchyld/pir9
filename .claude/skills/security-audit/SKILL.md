---
name: security-audit
description: Run all security tools and produce a consolidated report
user-invocable: true
disable-model-invocation: true
allowed-tools:
  - Read
  - Grep
  - Glob
  - Bash
---

# Security Audit

Run a comprehensive security scan of the pir9 codebase and produce a consolidated report.

## Scan Steps

Run these tools and collect findings:

### 1. Rust dependency vulnerabilities
```bash
cargo audit
```

### 2. Static analysis (SAST)
```bash
semgrep --config auto --json . 2>/dev/null | python3 -c "
import json, sys
data = json.load(sys.stdin)
for r in data.get('results', []):
    sev = r.get('extra', {}).get('severity', 'unknown')
    print(f\"[{sev.upper()}] {r['path']}:{r['start']['line']} - {r['check_id']}\")
    print(f\"  {r.get('extra', {}).get('message', '')}\")
" 2>/dev/null || semgrep --config auto .
```

### 3. Ecosystem CVE scan
```bash
grype dir:. --only-fixed
```

### 4. Secret detection
```bash
gitleaks detect --source . --no-git
```

### 5. SBOM inventory check
```bash
ls -la sbom.cdx.json  # Verify SBOM exists and is recent
```

## Report Format

Produce a summary organized by severity:

```
## Security Audit Report — YYYY-MM-DD

### Critical / High
- [finding details with file:line references]

### Medium
- [finding details]

### Low / Info
- [finding details]

### Dependency Summary
- Total dependencies: N
- Vulnerabilities found: N (critical: N, high: N, medium: N, low: N)

### Recommendations
1. [actionable fix for each critical/high finding]
```

## Notes

- This skill is **manual-only** (`disable-model-invocation: true`) — it should only run when explicitly invoked with `/security-audit`
- Some tools may not be installed — skip gracefully and note which tools were unavailable
- Focus recommendations on actionable fixes, not theoretical risks
- Reference the SBOM (`sbom.cdx.json`) for full dependency inventory
