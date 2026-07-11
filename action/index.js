'use strict'

const { exec } = require('node:child_process')
const { appendFile, mkdir, readFile } = require('node:fs/promises')
const path = require('node:path')
const { promisify } = require('node:util')

const execAsync = promisify(exec)

const OWNER = 'github-actions[bot]'
const EMAIL = '41898282+github-actions[bot]@users.noreply.github.com'

function getInput(name, fallback = '') {
  const key = `INPUT_${name.toUpperCase().replaceAll(' ', '_')}`
  return process.env[key] || fallback
}

function setOutput(name, value) {
  const outputFile = process.env.GITHUB_OUTPUT
  if (!outputFile) return

  const serialized = typeof value === 'string' ? value : JSON.stringify(value)
  return appendFile(outputFile, `${name}<<RIVET_OUTPUT\n${serialized}\nRIVET_OUTPUT\n`)
}

function info(message) {
  console.log(`::notice::${message}`)
}

function shellQuote(value) {
  return `'${value.replaceAll("'", "'\\''")}'`
}

async function run(command, cwd, ignoreFailure = false) {
  info(`Running: ${command}`)

  try {
    const result = await execAsync(command, {
      cwd,
      env: process.env,
      maxBuffer: 10 * 1024 * 1024,
      shell: '/bin/bash',
    })
    if (result.stdout) process.stdout.write(result.stdout)
    if (result.stderr) process.stderr.write(result.stderr)
    return { ...result, exitCode: 0 }
  } catch (error) {
    if (error.stdout) process.stdout.write(error.stdout)
    if (error.stderr) process.stderr.write(error.stderr)
    if (ignoreFailure) {
      return {
        stdout: error.stdout || '',
        stderr: error.stderr || '',
        exitCode: error.code || 1,
      }
    }
    throw error
  }
}

function apiUrl(pathname) {
  return `${process.env.GITHUB_API_URL || 'https://api.github.com'}${pathname}`
}

async function githubRequest(pathname, token, options = {}) {
  const response = await fetch(apiUrl(pathname), {
    ...options,
    headers: {
      Accept: 'application/vnd.github+json',
      Authorization: `Bearer ${token}`,
      'X-GitHub-Api-Version': '2022-11-28',
      ...options.headers,
    },
  })

  const text = await response.text()
  const data = text ? JSON.parse(text) : null
  if (!response.ok) {
    throw new Error(`GitHub API ${options.method || 'GET'} ${pathname} failed (${response.status}): ${data?.message || text}`)
  }
  return data
}

function repositoryParts() {
  const repository = process.env.GITHUB_REPOSITORY
  if (!repository || !repository.includes('/')) {
    throw new Error('GITHUB_REPOSITORY must be set to owner/name')
  }
  return repository.split('/', 2)
}

async function findVersionPullRequest(token, repository, baseBranch, branch) {
  const [owner] = repositoryParts()
  const query = new URLSearchParams({
    base: baseBranch,
    head: `${owner}:${branch}`,
    state: 'open',
  })
  const pullRequests = await githubRequest(`/repos/${repository}/pulls?${query}`, token)
  return pullRequests[0]
}

async function ensureVersionBranch({ token, cwd, repository, baseBranch, branch, command, commitMessage, setupGitUser }) {
  const remoteBranch = await githubRequest(
    `/repos/${repository}/branches/${encodeURIComponent(branch)}`,
    token,
  ).catch((error) => {
    if (error.message.includes('(404)')) return null
    throw error
  })

  if (setupGitUser === 'true') {
    await run(`git config user.name ${shellQuote(OWNER)} && git config user.email ${shellQuote(EMAIL)}`, cwd)
  }

  await run(`git fetch origin ${shellQuote(baseBranch)}`, cwd)
  if (remoteBranch) {
    await run(`git fetch origin ${shellQuote(branch)}`, cwd)
    await run(`git checkout -B ${shellQuote(branch)} origin/${shellQuote(branch)}`, cwd)
    await run(`git merge --no-edit origin/${shellQuote(baseBranch)}`, cwd)
  } else {
    await run(`git checkout -B ${shellQuote(branch)} origin/${shellQuote(baseBranch)}`, cwd)
  }

  await run(command, cwd)
  const changes = await run('git status --porcelain', cwd)
  if (!changes.stdout.trim()) return false

  await run('git add -A', cwd)
  await run(`git commit -m ${shellQuote(commitMessage)}`, cwd)
  await run(`git push origin HEAD:${shellQuote(branch)}`, cwd)
  return true
}

async function createOrUpdatePullRequest({ token, repository, baseBranch, branch, title, body }) {
  const existing = await findVersionPullRequest(token, repository, baseBranch, branch)
  if (existing) {
    await githubRequest(`/repos/${repository}/pulls/${existing.number}`, token, {
      body: JSON.stringify({ title, body }),
      headers: { 'Content-Type': 'application/json' },
      method: 'PATCH',
    })
    return existing.number
  }

  const pullRequest = await githubRequest(`/repos/${repository}/pulls`, token, {
    body: JSON.stringify({ base: baseBranch, body, head: branch, title }),
    headers: { 'Content-Type': 'application/json' },
    method: 'POST',
  })
  return pullRequest.number
}

async function configureNpmAuth() {
  const token = process.env.NPM_TOKEN
  if (!token) return

  const npmrc = path.join(process.env.HOME || process.cwd(), '.npmrc')
  await mkdir(path.dirname(npmrc), { recursive: true })
  const authLine = '//registry.npmjs.org/:_authToken='
  try {
    const existing = await readFile(npmrc, 'utf8')
    if (existing.split('\n').some((line) => line.trim().startsWith(authLine))) return
  } catch (error) {
    if (error.code !== 'ENOENT') throw error
  }
  await appendFile(npmrc, `${authLine}${token}\n`)
}

async function main() {
  const token = getInput('github-token', process.env.GITHUB_TOKEN)
  if (!token) throw new Error('A GitHub token is required. Pass github-token or set GITHUB_TOKEN.')
  await setOutput('published', 'false')

  if (
    process.env.GITHUB_EVENT_NAME &&
    !['push', 'workflow_dispatch'].includes(process.env.GITHUB_EVENT_NAME)
  ) {
    info(`Skipping Rivet release action for ${process.env.GITHUB_EVENT_NAME} event`)
    return
  }

  const cwd = path.resolve(getInput('cwd', '.'))
  const repository = process.env.GITHUB_REPOSITORY
  if (!repository) throw new Error('GITHUB_REPOSITORY must be set to owner/name')
  const baseBranch = getInput('base-branch', 'main')
  const branch = getInput('branch', 'rivet-release')
  const checkCommand = getInput('check', 'pnpm exec rivet check --json')
  const versionCommand = getInput('version', 'pnpm exec rivet bump')
  const publishCommand = getInput('publish')
  const check = await run(checkCommand, cwd, true)
  const output = `${check.stdout}\n${check.stderr}`
  const status = output
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith('{') && line.endsWith('}'))
    .map((line) => {
      try {
        return JSON.parse(line).status
      } catch {
        return undefined
      }
    })
    .find(Boolean)

  if (status === 'pending_releases' || output.includes('Release files exist')) {
    await setOutput('has-release', 'true')
    const changed = await ensureVersionBranch({
      branch,
      command: versionCommand,
      commitMessage: getInput('commit-message', 'chore: version packages'),
      cwd,
      repository,
      setupGitUser: getInput('setup-git-user', 'true'),
      baseBranch,
      token,
    })
    if (!changed) {
      info('Rivet found release files, but the version command produced no changes.')
      return
    }

    const number = await createOrUpdatePullRequest({
      baseBranch,
      body: getInput('pr-body', 'This PR was created automatically by Rivet.'),
      branch,
      repository,
      title: getInput('pr-title', 'chore: version packages'),
      token,
    })
    await setOutput('pull-request-number', String(number))
    info(`Created or updated Rivet version PR #${number}`)
    return
  }

  if (status === 'ready_to_release' || output.includes('Release plan exists')) {
    await setOutput('has-release', 'true')
    if (!publishCommand) {
      info('Rivet has a release plan, but no publish command was configured.')
      return
    }
    await configureNpmAuth()
    await run(publishCommand, cwd)
    await setOutput('published', 'true')
    info('Rivet published the prepared release.')
    return
  }

  if (check.exitCode !== 0) {
    throw new Error(`Rivet check failed with exit code ${check.exitCode}.`)
  }

  await setOutput('has-release', 'false')
  info('No Rivet release files or release plan found.')
}

if (require.main === module) {
  main().catch((error) => {
    console.error(`::error::${error.stack || error.message}`)
    process.exitCode = 1
  })
}

module.exports = { getInput, shellQuote }
