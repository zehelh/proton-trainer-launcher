# Dev Doctor Skill

## Purpose
Prevent infinite loops when troubleshooting ANY development issue. After 2 failed iterations on the same error, MANDATORY recap + web search. After 4 total attempts, STOP and ask user for manual research.

## Rule (NON-NEGOTIABLE)

**After 2 iterations on the SAME error:**

1. **STOP** - Do not attempt the same fix again
2. **RECAP** - Write a clear summary of:
   - The exact error message
   - What has been tried (list all attempts)
   - Current state of files/configuration
3. **WEB SEARCH** - Use web search tool to find solutions from:
   - Official documentation
   - Stack Overflow
   - GitHub issues
   - Relevant forums
4. **APPLY** - Try the most promising solution from search
5. **IF STILL FAILING after 2 more attempts (4 total):**
   - Send the recap to chat
   - State: "I've tried X times, here's the full recap, please research manually"
   - DO NOT continue guessing

## Counter Tracking

Maintain a counter for each error type in scratchpad:
```
Error: "[Exact error message]"
- Attempt 1: [What you tried] - Result: [What happened]
- Attempt 2: [What you tried] - Result: [What happened]
- Attempt 3: [STOP - recap + web search]
- Attempt 4: [SEND TO USER - manual research needed]
```

## What Counts as "Same Error"

Errors are considered the same if they have:
- Same error message (ignoring timestamps, container IDs, file paths)
- Same root cause
- Same error code/type

## What to Include in Recap

When you hit attempt 3 (first recap + search):

```markdown
## Error Recap - Attempt 3

**Error Type**: [Rust / Build / Runtime]

**Error**: [Exact error message]

**Context**:
- File: [file path]
- Line: [line number if applicable]
- Command that triggered

**Attempts Made**:
1. [What you tried] - Result: [What happened]
2. [What you tried] - Result: [What happened]

**Current State**:
- [List of files modified]
- [Current configuration snippets]
- [Relevant file contents]

**Web Search Query**: [What you searched for]

**Solution Found**: [What you're trying based on search]
```

## When to Send to User

Send to user when:
- Same error occurs for the 4th time
- OR you've exhausted all solutions from web search
- OR the error is clearly outside your capability
- OR you've spent more than 10 minutes on the same issue

**Message format:**
```
I've hit the same error 4 times. Here's the complete recap:

[PASTE RECAP HERE]

I've tried all solutions from my knowledge base and web search. 
Please research manually and provide guidance.
```

## Skill Execution Flow

```
User reports issue
       │
       ▼
┌─────────────────────┐
│  Phase 1: Diagnosis  │ ◄── No (Same error ×2) ───┐
└─────────┬───────────┘                         │
          │                                       │
          ▼                                       ▼
┌─────────────────────┐               ┌─────────────────────┐
│  Apply quick fixes   │               │  Phase 2: Investigate │
│  (config, imports,   │               │  (Deep dive into     │
│   types, etc.)       │               │   files, configs)     │
└─────────┬───────────┘               └─────────┬───────────┘
          │                                       │
          ▼                                       ▼
┌─────────────────────┐               ┌─────────────────────┐
│  Test fix            │               │  Same error again?   │
└─────────┬───────────┘               └─────────┬───────────┘
          │                                       │
          ▼ No                                   ▼ Yes
    Resolved!                               Phase 3: Web Search
                                          │
                                          ▼
                                Search for solutions
                                          │
                                          ▼
                                Apply and test
                                          │
                                          ▼
                                If still failing:
                                - Check attempt count
                                - If < 4: Continue
                                - If >= 4: STOP and send to user
```

## Tools to Use

```typescript
// Use search_tool_functions to find web search
const { functions } = await tools.search_tool_functions({
  mode: "best_match",
  query: "web search"
});

// Then use the web search function
const results = await tools[functions[0]]({ query: "your search query" });
```

## Best Practices

1. **Always read the full error message** - Don't just look at the first line
2. **Check related files** - Errors often cascade from one file to another
3. **Test incrementally** - Fix one error at a time, rebuild/test after each
4. **Use scratchpad** - Document your attempts for transparency
5. **Verify before committing** - Make sure the fix actually resolves the issue
6. **Clean up** - Remove old containers, clear caches, rebuild from scratch if needed


## What NOT to Do

❌ Do NOT keep trying the same fix with slight variations
❌ Do NOT make more than 2 attempts without web search
❌ Do NOT hide the recap from the user
❌ Do NOT continue after 4 failed attempts on the same error
❌ Do NOT guess at type annotations without checking the actual types
❌ Do NOT modify files you haven't read first

## Summary

**Rule**: 2 attempts → recap + search. 4 attempts → stop and send to user.

This prevents infinite loops and ensures you either:
1. Fix the issue within 2-3 attempts, OR
2. Escalate to user with full context for manual research

Apply this to ALL development issues.
