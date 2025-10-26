# Agent Guidelines

- Initialize the repository with the default branch named `master`, not `main`.
- For each user request, create a dedicated topic branch and prepare a pull request; if a new request continues prior work, keep using the same topic branch.
- Before starting a new topic branch, fast-forward `master` to `origin/master` so local history matches the remote.
- We are developing a Rust CLI application in this workspace.
