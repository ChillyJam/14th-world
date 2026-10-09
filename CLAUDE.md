# Working on issues

When you pick up a GitHub issue, mark it as in progress before you start:

```sh
gh issue edit <number> --add-label "in progress"
```

Put `Closes #<number>` in the pull request description. When the pull request
is merged the issue closes and the
[In progress label](.github/workflows/in-progress-label.yml) workflow removes
the label. The same workflow also adds the label when a pull request that
closes the issue is opened, in case it was missed.
