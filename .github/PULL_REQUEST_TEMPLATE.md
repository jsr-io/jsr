### PR Checklist

- [ ] The PR title follows
      [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/)
- [ ] Is this closing an open issue? If so, link it, else include a proper
      description of the changes and rason behind them.
- [ ] Does the PR have changes to the frontend? If so, include screenshots or a
      recording of the changes.
      <br/>If it affect colors, please include screenshots/recording in both
      light and dark mode.
- [ ] Does the PR have changes to the backend? If so, make sure tests are added.
      <br/>And if changing dababase queries, be sure you have ran `sqlx prepare`
      and committed the changes in the `.sqlx` directory.
