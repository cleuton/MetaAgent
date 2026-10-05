# Programming guide

How to make decisions, repeat things, keep results, and put them together. Every example here was run.

## Values

Text `"hello"`, numbers `3`, `yes` / `no`, `nothing`, lists `["a", "b"]`, and records (what tools give
back, such as `clock.now`). A name holds a value: `city = "Lisbon"`.

## Getting a result

A call gives back a value. Keep it with `=`:

```text
now = clock.now
page = http.get url: "https://example.org"
```

Records have fields, which you reach with a dot:

```text
reply "{now.text} ({now.unix})"
reply "status {page.status}"
```

A message's values (`accepts ask city`) are already names inside its `on ask`.

## Making decisions: `if` and `otherwise`

```text
if n is more than 10 and not word is "skip"
  reply "big"
otherwise
  if word contains "ab" or n is less than 0
    reply "match"
  otherwise
    reply "small"
```

Tests you can use:

| Write | Means |
|-------|-------|
| `a is b`, `a is not b` | equal, not equal (`"5"` and `5` count as equal) |
| `a is more than b`, `a is less than b` | compare two numbers |
| `text contains "x"`, `list contains item` | is it inside |
| `x and y`, `x or y`, `not x` | combine tests |

`otherwise` is optional. `nothing`, an empty text, an empty list, `no` and `0` count as false, so
`if answer` means "if there is an answer".

## Repeating: `for`

```text
for city in ["Lisbon", "Porto", "Faro"]
  answer = Weather.ask city: city
```

The list can come from anywhere: a literal, a record field that holds a list (`page.json.items`), or
a name.

## Putting results together

There is no `+` and no "add to a list". You put results together inside text, which can mention
its own earlier value:

```text
report = "Weather report:"
for city in ["Lisbon", "Porto", "Faro"]
  answer = Weather.ask city: city
  report = "{report}\n- {city}: {answer}"
reply report
```

This prints:

```text
Weather report:
- Lisbon: sunny
- Porto: rainy
- Faro: rainy
```

`\n` is a new line. To remember something across the loop, or across requests, use the agent's memory:

```text
tool state
...
  if answer is "sunny"
    state.set key: "last-sunny" value: city
...
last = state.get key: "last-sunny"
if last is nothing
  reply "{report}\nNo sunny city."
otherwise
  reply "{report}\nLast sunny city: {last}"
```

To turn many results into one summary, hand them to the model. Its answer is a value like any other:

```text
summary = think "Summarize this in one sentence:\n{report}"
reply summary
```

## Stopping on purpose

```text
if city is nothing
  fail "I need a city"
```

`fail` stops the agent and shows your sentence, with the line, as the error.

## Splitting work between agents

Give each agent one job and call it like a tool (see `link` in the tutorial). A linked agent's `reply`
is the value the caller gets. That is the way to "make a function".

## What the language does not have (yet)

Being small on purpose (a beginner must be able to read it aloud), it has no arithmetic (`+`, `-`,
counting), no way to add to a list, no functions of your own, and no way to catch an error and carry
on. To count or calculate, use a tool: an MCP server that does math, a linked agent written for the
purpose, or `think`.

## Where results go

- `reply` sends the answer back to whoever asked (the terminal, another agent, an A2A client).
- `file.write path: "report.txt" text: report` saves it (needs `tool file`).
- `http.post url: "..." body: report` sends it somewhere (needs `tool http`).
