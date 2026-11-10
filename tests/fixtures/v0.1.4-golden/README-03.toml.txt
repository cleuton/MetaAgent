exit: 1
--- stdout
--- stderr
Warning: [auth] is set but [version] is not. Metagente 0.1.3 and older ignore [auth] and would serve every message without protection. Add a [version] section with minimum = "<version>".
Warning: [auth] forward lists Researcher, but nothing with that name is declared (list a remote agent or a tool reached by an http:// or https:// address)
Warning: [auth] forward lists kb, but nothing with that name is declared (list a remote agent or a tool reached by an http:// or https:// address)
Problem on line 3 of <tmp>/metagente.toml: [auth] protect lists Planner.plan, but no such message is declared.
Fix: check the agent name in your .ag files

Problem on line 3 of <tmp>/metagente.toml: [auth] protect lists Worker.*, but no such message is declared.
Fix: check the agent name in your .ag files

Found 2 problems.
