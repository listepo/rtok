# SonarCloud OSS setup (rtok)

Maintainer guide for the GitHub Actions workflow in `.github/workflows/sonarcloud.yml`.
Analysis runs on **push to `main`** and on **`workflow_dispatch`** (not on pull requests).

The `sonar.organization` / `sonar.projectKey` values in `sonar-project.properties`
(`listepo` / `listepo_rtok`) are **placeholders** until they match the SonarCloud UI
after you import the project.

## 1. Create the SonarCloud project

1. Sign in at [sonarcloud.io](https://sonarcloud.io) with GitHub.
2. Choose **Get SonarQube for OSS** (or the equivalent free OSS plan) for the
   **listepo** GitHub organization.
3. Import the **listepo/rtok** repository.
4. Note the **organization key** and **project key** shown in the UI. If they differ
   from `listepo` / `listepo_rtok`, update `sonar-project.properties` so they match
   exactly — a mismatch produces empty or orphaned analyses.

## 2. Turn Automatic Analysis OFF

SonarCloud’s **Automatic Analysis** must be **disabled** for this project. The CI
scanner (`SonarSource/sonarqube-scan-action`) owns analysis, including coverage from
`cargo llvm-cov`. Leaving Automatic Analysis on duplicates work and ignores the LCOV
file produced in Actions.

In the SonarCloud project: **Administration → Analysis Method → Automatic Analysis → Off**.

## 3. Generate `SONAR_TOKEN`

1. SonarCloud → **My Account → Security → Generate Tokens**.
2. Create a token with permission to analyze this project (name it e.g. `rtok-github-actions`).
3. Copy the value once — it is shown only at creation time.

## 4. Add the GitHub Actions secret

On **listepo/rtok**:

**Settings → Secrets and variables → Actions → New repository secret**

- Name: `SONAR_TOKEN`
- Value: the token from step 3

Do not commit the token. The workflow reads `${{ secrets.SONAR_TOKEN }}` only.
If the secret is missing (forks, clones without secrets), the scan step **soft-skips**
with a notice so the job stays green.

## 5. First analysis after merge

1. Merge the SonarCloud PR (or push to `main`).
2. Open the **Actions** tab → workflow **sonarcloud** → confirm fmt, clippy, llvm-cov,
   and the Sonar scan succeeded.
3. Open the SonarCloud project dashboard — the first analysis should appear for `main`.

PR decoration (inline comments on pull requests) needs the SonarCloud GitHub App
installed on the org/repo **and** a workflow that runs on `pull_request`. This repo’s
workflow intentionally runs only on `main` push / `workflow_dispatch`; enable PR
triggers later if you want decoration.

## 6. Coverage (`sonar.rust.lcov.reportPaths`)

The workflow:

1. Installs `llvm-tools-preview` and `cargo-llvm-cov`.
2. Runs `cargo llvm-cov nextest --workspace --lcov --output-path coverage/lcov.info` —
   nextest (from `mise.toml`) like `just test`; plain `cargo test` runs every test in one
   process, where the proxy tests exhaust httpmock's server pool and hang.
3. Points Sonar at that file via **`sonar.rust.lcov.reportPaths`** in
   `sonar-project.properties` (not `sonar.coverageReportPaths`).

`crates/rtok-webui` is already outside the Cargo workspace and is listed under
`sonar.exclusions`.

### Follow-ups (optional)

If llvm-cov is too slow or fragile on CI:

- Exclude heavy integration tests with llvm-cov `--ignore-filename-regex` or a narrower
  package set.
- Cache `target/` more aggressively (rust-cache is already enabled).
- Keep producing a real `coverage/lcov.info`; empty or missing LCOV just means no
  coverage on the dashboard, not a hard Sonar failure by default.

## 7. Local dry-run (optional)

```bash
mise install
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
mkdir -p coverage
mise exec -- cargo llvm-cov nextest --workspace --lcov --output-path coverage/lcov.info
# Then run the Sonar scanner locally only if you have SONAR_TOKEN exported —
# never write the token into the repo.
```

## References

- Workflow: `.github/workflows/sonarcloud.yml`
- Properties: `sonar-project.properties`
- Official CI action: `SonarSource/sonarqube-scan-action` (current; prefer over the
  legacy `sonarcloud-github-action`)
