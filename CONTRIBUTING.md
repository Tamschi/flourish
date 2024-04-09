# Contribution Guidelines

## Issues

This repository contains issue templates for [bugs] and [feature requests].  
Unclear documentation or error messages are considered bugs.

For anything else, please use the ["Custom issue"] template.

[bugs]: https://github.com/Tamschi/flourish-pollinate/issues/new?assignees=&labels=bug&template=bug_report.md&title=
[feature requests]: https://github.com/Tamschi/flourish-pollinate/issues/new?assignees=&labels=enhancement&template=feature_request.md&title=
["Custom issue"]: https://github.com/Tamschi/flourish-pollinate/issues/new?assignees=&labels=&template=custom_issue.md&title=

## Pull Requests

### CI

This repository uses fairly extensive CI to make sure everything is in order.  
GitHub Actions will automatically build and test your pull requests.

**I recommend working on branches with a `-` or `/` in their name.**  
The CI is configured slightly differently for them to make WIP code a bit easier.

Additionally, when you run `cargo test` for the first time, [cargo-husky] sets up a Git pre-push hook to run tests.  
This includes a branch name check, which is ignored on any branches that have a `-` or '/' in their name.  
You can still push failing builds using `git push --no-verify`.

Warnings are only denied on `develop`, but the CI should still detect them for pull requests towards that branch.

[cargo-husky]: https://lib.rs/crates/cargo-husky

### Code Style

Please keep your code human-readable.

While there are no formal style requirements, here are some suggestions that might help new code fit in with with the existing:

- Don't use abbreviations unless they are established terms.

  They usually make it harder for me to read the code fluently. You also don't need to worry about alignment; I use a proportional font and likely wouldn't notice.

- Try to keep it simple.

  I can't properly review code I don't understand, so straightforward implementations are usually preferred.

  It's usually fine to use a library to avoid boilerplate, if there's enough documentation so I could replicate it from scratch.

  If you do something custom that's tricky, a link to an explanation of the technique would be nice. I'll just ask if this becomes an issue, though.

- If you use macros, put them nearby.

  I normally place one-off macros directly above the item I need them for.

- When writing comments and documentation, try to break lines semantically.

  It's more readable than breaking purely by line width this way, in my eyes.

  Just after commas tends to be a good place for this, of course,
  but please try to also take sentence flow into account when there are none.
  If there are multiple sentences in a paragraph, you may want to put them in separate lines.

  Please do not manually break lines in the rendered documentation (trailing double-space), unless this strongly improved legibility.

### Meta data

Please add yourself to each copyright holders list of [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) when contributing, or alternatively include a note in your pull request that you intentionally didn't do so.

Nicknames and entries without email addresses are fine, too.

For substantial contributions (basically anything more than typo or grammar fixes), feel free to add yourself to the `authors` list in `Cargo.toml`. This explicitly includes documentation changes, testing and bug fixes that just happen to be not much code.

### Optional: Update [CHANGELOG.md](CHANGELOG.md)

I use the following format for an upcoming release with contributed changes:

```markdown
## next

TODO: Date

- Revisions:
  - Change title (contributed by @<your GitHub @> in #<PR #>)
    > Further description or motivation, if necessary.
```

When adding your change, replace:

- `Revisions` with `Features` or `**Breaking changes**`, iff your contribution falls into one of those categories instead.
- `<your GitHub @>` with your GitHub username.
- `<PR #>` with the id of your pull-request. (Squashing is optional)
- `Change title` and `> Further description…` as appropriate.

See non-contributed changes from earlier releases for examples.

## Labels

Don't worry about these too much.

You're encouraged to add any that seem applicable where possible,
but I'll otherwise just take care of it.

(Don't undo changes I make to labels without immediate reason.)

See <https://github.com/Tamschi/flourish-pollinate/issues/labels> for details on individual labels.

### Categories

- Assorted

  Labels without prefix like [`breaking`](https://github.com/Tamschi/flourish-pollinate/labels/breaking),
  [`good first issue`](https://github.com/Tamschi/flourish-pollinate/labels/good%20first%20issue) or
  [`help wanted`](https://github.com/Tamschi/flourish-pollinate/labels/help%20wanted).

- `domain:`

  Categorises changes by domain. Mostly not necessary.

- `effort:`

  Relative effort required. There's no specific unit of measurement.

- `priority:`

  Vaguely informs my schedule, **cross-repository**.

  You're welcome to let me know that (and ideally why) you'd like to see a specific change and I'll take that into account.

  If you *need* a feature that you're not planning to implement yourself, strongly consider paying me for it.

  > This is, of course, subject to side-job restrictions I may be under.

  <!---->

  > If you'd like to pay me directly, contact me first and we'll figure out how to do this as industry-standard contract work.
  >
  > Alternatively:
  >
  > For crowdfunding and escrow, [Bountysource](https://www.bountysource.com/) seems reasonably trustworthy. This also has the advantage of letting someone else work on it, since I'm usually pretty swamped with projects.
  >
  > Use fiat bounties. Cryptoscam "currencies" are a scourge.
  >
  > Posting a bounty won't guarantee I'll actually implement a solution, but I'll try to speedily triage relevant issues at least.
  >
  > I'll try to set up something that automatically announces bounties,
  > but if that doesn't happen within a few hours, **do** post a comment about it!

- `state:`

  General scheduling categories. See label descriptions for details!

  Rarely, more than one may be applicable.

- `type:`

  General problem or change domain categories.

  Only one of these should be used at a time.

- `work:`

  These are inspired by the [Cynefin framework](https://en.wikipedia.org/wiki/Cynefin_framework) to categorise the type of work done or required.

  I adjusted the labels to be a bit friendlier and more expressive. See the label descriptions for details.

  The unknown category at the centre is not labelled explicitly.
