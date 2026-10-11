import { useEffect, useRef, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeRaw from 'rehype-raw'
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize'
import {
  GitBranch, GitCommitHorizontal, FolderGit2, FolderOpen, ChevronDown, ChevronLeft, Download, Copy,
  History, Leaf, RefreshCw, Settings2, Sparkles, AlertCircle, FileCode2, ChevronRight, Plus, Minus,
  Search, LockKeyhole, ExternalLink, Library, ArrowDownToLine, ArrowUpFromLine,
} from 'lucide-react'
import './App.css'

type Repository = { path: string; name: string; branch: string }
type Commit = { hash: string; shortHash: string; message: string; author: string; authoredAt: string }
type CommitDetails = Commit & { files: string[] }
type ChangedFile = { path: string; stagedStatus: string | null; unstagedStatus: string | null }
type FileDiff = { path: string; staged: boolean; text: string }
type SelectedFile = { path: string; staged: boolean }
type Branch = { name: string; current: boolean }
type GithubUser = { login: string; avatarUrl: string; htmlUrl: string }
type GithubRepository = { id: number; name: string; fullName: string; description: string | null; private: boolean; htmlUrl: string; cloneUrl: string; defaultBranch: string; updatedAt: string; fork: boolean }
type GithubSearchResults = { items: GithubRepository[]; totalCount: number; page: number }
const RECENTS_KEY = 'gitmint.recent-repositories'
const LIBRARY_TAB_KEY = 'gitmint.default-library-tab'
const GITHUB_CLIENT_ID = import.meta.env.VITE_GITHUB_CLIENT_ID ?? ''
const AUTH_BROKER_URL = import.meta.env.VITE_GITMINT_AUTH_BROKER_URL ?? ''

function App() {
  const [githubUser, setGithubUser] = useState<GithubUser | null>(null)
  const [githubRepositories, setGithubRepositories] = useState<GithubRepository[]>([])
  const [githubRepositoriesLoading, setGithubRepositoriesLoading] = useState(false)
  const [githubSearchResults, setGithubSearchResults] = useState<GithubSearchResults | null>(null)
  const [githubSearchLoading, setGithubSearchLoading] = useState(false)
  const [githubSearchFailed, setGithubSearchFailed] = useState(false)
  const [selectedGithubRepository, setSelectedGithubRepository] = useState<GithubRepository | null>(null)
  const [repositoryReadme, setRepositoryReadme] = useState<string | null>(null)
  const [repositoryReadmeError, setRepositoryReadmeError] = useState('')
  const [repositoryReadmeLoading, setRepositoryReadmeLoading] = useState(false)
  const [cloningRepository, setCloningRepository] = useState<number | null>(null)
  const [libraryTab, setLibraryTab] = useState<'github' | 'local'>(() => localStorage.getItem(LIBRARY_TAB_KEY) === 'local' ? 'local' : 'github')
  const [repositorySearch, setRepositorySearch] = useState('')
  const [authChecking, setAuthChecking] = useState(true)
  const [authBusy, setAuthBusy] = useState(false)
  const [authError, setAuthError] = useState('')
  const [repository, setRepository] = useState<Repository | null>(null)
  const [recent, setRecent] = useState<Repository[]>(() => {
    try { return JSON.parse(localStorage.getItem(RECENTS_KEY) ?? '[]') as Repository[] }
    catch { return [] }
  })
  const [pathInput, setPathInput] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [commits, setCommits] = useState<Commit[]>([])
  const [selectedCommit, setSelectedCommit] = useState<string | null>(null)
  const [commitDetails, setCommitDetails] = useState<CommitDetails | null>(null)
  const [loadingHistory, setLoadingHistory] = useState(false)
  const [activeView, setActiveView] = useState<'repositories' | 'history' | 'changes' | 'branches' | 'settings'>('repositories')
  const [changes, setChanges] = useState<ChangedFile[]>([])
  const [selectedFile, setSelectedFile] = useState<SelectedFile | null>(null)
  const [fileDiff, setFileDiff] = useState<FileDiff | null>(null)
  const [loadingChanges, setLoadingChanges] = useState(false)
  const [loadingDiff, setLoadingDiff] = useState(false)
  const [operationBusy, setOperationBusy] = useState(false)
  const [syncBusy, setSyncBusy] = useState(false)
  const [commitMessage, setCommitMessage] = useState('')
  const [notice, setNotice] = useState('')
  const [branches, setBranches] = useState<Branch[]>([])
  const [branchName, setBranchName] = useState('')
  const githubRepositoryRequest = useRef(0)
  const githubSearchRequest = useRef(0)
  const repositoryReadmeRequest = useRef(0)
  const openRepositoryRef = useRef<(path?: string) => Promise<void>>(async () => {})

  useEffect(() => { localStorage.setItem(RECENTS_KEY, JSON.stringify(recent)) }, [recent])
  useEffect(() => { localStorage.setItem(LIBRARY_TAB_KEY, libraryTab) }, [libraryTab])
  useEffect(() => {
    if (!error) return
    const timeout = window.setTimeout(() => setError(''), 5000)
    return () => window.clearTimeout(timeout)
  }, [error])
  useEffect(() => {
    if (!notice) return
    const timeout = window.setTimeout(() => setNotice(''), 5000)
    return () => window.clearTimeout(timeout)
  }, [notice])
  useEffect(() => {
    let active = true
    invoke<GithubUser | null>('github_session')
      .then((user) => { if (active) setGithubUser(user) })
      .catch((reason: unknown) => { if (active) setAuthError(String(reason)) })
      .finally(() => { if (active) setAuthChecking(false) })
    return () => { active = false }
  }, [])
  useEffect(() => {
    if (githubUser) void refreshGithubRepositories()
  }, [githubUser])
  useEffect(() => {
    const query = repositorySearch.trim()
    if (libraryTab !== 'github' || query.length < 2) {
      githubSearchRequest.current += 1
      return
    }
    const timeout = window.setTimeout(() => { void searchGithubRepositories(query) }, 350)
    return () => {
      window.clearTimeout(timeout)
      githubSearchRequest.current += 1
    }
  }, [libraryTab, repositorySearch])
  useEffect(() => {
    if (!repository) return
    let current = true
    invoke<Commit[]>('list_commits', { path: repository.path })
      .then((items) => { if (current) { setCommits(items); setSelectedCommit(items[0]?.hash ?? null) } })
      .catch((reason: unknown) => { if (current) setError(String(reason)) })
      .finally(() => { if (current) setLoadingHistory(false) })
    return () => { current = false }
  }, [repository])
  useEffect(() => {
    if (!repository || !selectedCommit) return
    let current = true
    invoke<CommitDetails>('get_commit_details', { path: repository.path, hash: selectedCommit })
      .then((details) => { if (current) setCommitDetails(details) })
      .catch((reason: unknown) => { if (current) setError(String(reason)) })
    return () => { current = false }
  }, [repository, selectedCommit])
  useEffect(() => {
    const onShortcut = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'o') {
        event.preventDefault()
        void openRepositoryRef.current()
      }
    }
    window.addEventListener('keydown', onShortcut)
    return () => window.removeEventListener('keydown', onShortcut)
  }, [])

  async function openRepository(path?: string) {
    if (!githubUser) return
    setError('')
    let selectedPath = path
    if (!selectedPath) {
      const result = await open({ directory: true, multiple: false, title: 'Open a Git repository' })
      if (typeof result !== 'string') return
      selectedPath = result
    }
    setBusy(true)
    try {
      const info = await invoke<Repository>('inspect_repository', { path: selectedPath })
      setLoadingHistory(true)
      setCommits([])
      setSelectedCommit(null)
      setCommitDetails(null)
      setRepository(info)
      setActiveView('history')
      if (activeView === 'changes') void refreshChanges(info.path)
      if (activeView === 'branches') void refreshBranches(info.path)
      setPathInput('')
      setRecent((items) => [info, ...items.filter((item) => item.path !== info.path)])
    } catch (reason) {
      setError(String(reason))
    } finally { setBusy(false) }
  }

  async function refreshGithubRepositories() {
    const requestId = ++githubRepositoryRequest.current
    setGithubRepositoriesLoading(true)
    setError('')
    try {
      const repositories = await invoke<GithubRepository[]>('list_github_repositories')
      if (requestId === githubRepositoryRequest.current) {
        setGithubRepositories(repositories)
        setError('')
      }
    } catch (reason) {
      if (requestId === githubRepositoryRequest.current) setError(String(reason).replace(/^Error: /, ''))
    } finally {
      if (requestId === githubRepositoryRequest.current) setGithubRepositoriesLoading(false)
    }
  }

  async function searchGithubRepositories(query: string, page = 1, append = false) {
    const requestId = ++githubSearchRequest.current
    setGithubSearchLoading(true)
    if (!append) {
      setError('')
      setGithubSearchFailed(false)
    }
    try {
      const results = await invoke<GithubSearchResults>('search_github_repositories', { query, page })
      if (requestId === githubSearchRequest.current) {
        setGithubSearchResults((current) => append && current ? { ...results, items: [...current.items, ...results.items] } : results)
      }
    } catch (reason) {
      if (requestId === githubSearchRequest.current) {
        setError(String(reason).replace(/^Error: /, ''))
        if (!append) setGithubSearchFailed(true)
      }
    } finally {
      if (requestId === githubSearchRequest.current) setGithubSearchLoading(false)
    }
  }

  async function showGithubRepository(item: GithubRepository) {
    setSelectedGithubRepository(item)
    setRepositoryReadme(null)
    setRepositoryReadmeError('')
    setRepositoryReadmeLoading(true)
    setError('')
    const requestId = ++repositoryReadmeRequest.current
    try {
      const readme = await invoke<string | null>('get_github_repository_readme', { fullName: item.fullName })
      if (requestId === repositoryReadmeRequest.current) setRepositoryReadme(readme)
    } catch (reason) {
      if (requestId === repositoryReadmeRequest.current) {
        const message = String(reason).replace(/^Error: /, '')
        setError(message)
        setRepositoryReadmeError(message)
      }
    } finally {
      if (requestId === repositoryReadmeRequest.current) setRepositoryReadmeLoading(false)
    }
  }

  async function copyRepositoryUrl(url: string) {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(url)
      } else {
        const input = document.createElement('textarea')
        input.value = url
        input.style.position = 'fixed'
        input.style.opacity = '0'
        document.body.append(input)
        input.select()
        const copied = document.execCommand('copy')
        input.remove()
        if (!copied) throw new Error('Clipboard access is unavailable.')
      }
      setNotice('Repository URL copied')
    } catch {
      setError('Could not copy the repository URL. Please copy it from GitHub instead.')
    }
  }

  async function cloneGithubRepository(item: GithubRepository) {
    const parent = await open({ directory: true, multiple: false, title: `Choose where to clone ${item.name}` })
    if (typeof parent !== 'string') return
    setCloningRepository(item.id)
    setError('')
    setNotice('')
    try {
      const cloned = await invoke<Repository>('clone_github_repository', { cloneUrl: item.cloneUrl, parentPath: parent })
      setRecent((items) => [cloned, ...items.filter((entry) => entry.path !== cloned.path)])
      await openRepository(cloned.path)
      setNotice(`Cloned ${item.fullName} and opened it`)
    } catch (reason) {
      setError(String(reason).replace(/^Error: /, ''))
    } finally {
      setCloningRepository(null)
    }
  }

  async function refreshChanges(path = repository?.path) {
    if (!path) return
    setLoadingChanges(true)
    setError('')
    try {
      const items = await invoke<ChangedFile[]>('list_changes', { path })
      setChanges(items)
      const next = items.find((item) => item.unstagedStatus) ?? items.find((item) => item.stagedStatus)
      if (!next) {
        setSelectedFile(null)
        setFileDiff(null)
      } else {
        const staged = !next.unstagedStatus && Boolean(next.stagedStatus)
        await selectFile(next.path, staged, path)
      }
    } catch (reason) {
      setError(String(reason))
    } finally { setLoadingChanges(false) }
  }

  async function selectFile(filePath: string, staged: boolean, repoPath = repository?.path) {
    if (!repoPath) return
    setSelectedFile({ path: filePath, staged })
    setLoadingDiff(true)
    setFileDiff(null)
    try {
      const result = await invoke<FileDiff>('get_file_diff', { path: repoPath, filePath, staged })
      setFileDiff(result)
    } catch (reason) {
      setError(String(reason))
    } finally { setLoadingDiff(false) }
  }

  async function stageFile(file: ChangedFile, staged: boolean) {
    if (!repository) return
    setOperationBusy(true)
    setError('')
    setNotice('')
    try {
      await invoke(staged ? 'unstage_file' : 'stage_file', { path: repository.path, filePath: file.path })
      await refreshChanges(repository.path)
    } catch (reason) {
      setError(String(reason))
    } finally { setOperationBusy(false) }
  }

  async function stageAllChanges() {
    if (!repository) return
    setOperationBusy(true)
    setError('')
    setNotice('')
    try {
      await invoke('stage_all', { path: repository.path })
      await refreshChanges(repository.path)
    } catch (reason) {
      setError(String(reason))
    } finally { setOperationBusy(false) }
  }

  async function refreshHistory(path = repository?.path) {
    if (!path) return
    const items = await invoke<Commit[]>('list_commits', { path })
    setCommits(items)
    setSelectedCommit(items[0]?.hash ?? null)
  }

  async function refreshBranches(path = repository?.path) {
    if (!path) return
    try {
      setBranches(await invoke<Branch[]>('list_branches', { path }))
    } catch (reason) {
      setError(String(reason))
    }
  }

  async function createBranch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!repository || !branchName.trim()) return
    setOperationBusy(true)
    setError('')
    setNotice('')
    try {
      await invoke('create_branch', { path: repository.path, name: branchName })
      setBranchName('')
      await refreshBranches(repository.path)
      setNotice('Branch created')
    } catch (reason) {
      setError(String(reason))
    } finally { setOperationBusy(false) }
  }

  async function switchBranch(name: string) {
    if (!repository) return
    setOperationBusy(true)
    setError('')
    setNotice('')
    try {
      const branch = await invoke<string>('switch_branch', { path: repository.path, name })
      setRepository({ ...repository, branch })
      await refreshBranches(repository.path)
      setNotice(`Switched to ${branch}`)
    } catch (reason) {
      setError(String(reason))
    } finally { setOperationBusy(false) }
  }

  async function commitChanges(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!repository) return
    setOperationBusy(true)
    setError('')
    setNotice('')
    try {
      const hash = await invoke<string>('create_commit', { path: repository.path, message: commitMessage })
      setCommitMessage('')
      await refreshChanges(repository.path)
      await refreshHistory(repository.path)
      setNotice(`Commit ${hash} created`)
    } catch (reason) {
      setError(String(reason))
    } finally { setOperationBusy(false) }
  }

  async function syncRepository(direction: 'pull' | 'push') {
    if (!repository) return
    setSyncBusy(true)
    setError('')
    setNotice('')
    try {
      const message = await invoke<string>(direction === 'pull' ? 'pull_repository' : 'push_repository', { path: repository.path })
      await refreshHistory(repository.path)
      await refreshChanges(repository.path)
      setNotice(message)
    } catch (reason) {
      setError(String(reason).replace(/^Error: /, ''))
    } finally {
      setSyncBusy(false)
    }
  }

  const globalSearchActive = libraryTab === 'github' && repositorySearch.trim().length >= 2
  const displayedGithubRepositories = globalSearchActive
    ? githubSearchResults?.items ?? []
    : githubRepositories.filter((item) => `${item.fullName} ${item.description ?? ''}`.toLowerCase().includes(repositorySearch.toLowerCase()))

  useEffect(() => { openRepositoryRef.current = openRepository })

  async function signInWithGithub() {
    setAuthBusy(true)
    setAuthError('')
    try {
      const user = await invoke<GithubUser>('sign_in_github', { clientId: GITHUB_CLIENT_ID, brokerUrl: AUTH_BROKER_URL })
      setGithubUser(user)
    } catch (reason) {
      setAuthError(String(reason).replace(/^Error: /, ''))
    } finally { setAuthBusy(false) }
  }

  async function signOut() {
    githubRepositoryRequest.current += 1
    githubSearchRequest.current += 1
    repositoryReadmeRequest.current += 1
    try { await invoke('sign_out_github') }
    catch (reason) { setError(String(reason)); return }
    setError('')
    setGithubUser(null)
    setGithubSearchResults(null)
    setSelectedGithubRepository(null)
    setRepositoryReadme(null)
    setRepository(null)
    setCommits([])
    setChanges([])
    setBranches([])
  }

  if (authChecking) return <main className="auth-screen"><div className="auth-card"><Brand /><div className="auth-spinner" /><p>Checking your GitHub sign-in…</p></div></main>
  if (!githubUser) return <main className="auth-screen"><section className="auth-card">
    <Brand />
    <div className="auth-icon"><GithubMark size={26} /></div>
    <div className="auth-kicker">GITHUB REQUIRED</div>
    <h1>Your Git workflow,<br /><em>refreshed.</em></h1>
    <p>Sign in with GitHub to browse your repositories. Local Git operations stay on this Mac.</p>
    {authError && <div className="auth-error" role="alert"><AlertCircle size={15} /><span>{authError}</span></div>}
    <button className="github-signin-button" onClick={() => void signInWithGithub()} disabled={authBusy}>
      <GithubMark size={17} />{authBusy ? 'Waiting for GitHub…' : 'Continue with GitHub'}
    </button>
    <span className="auth-footnote">GitHub access lets GitMint list repositories and authenticate HTTPS clone, push, and pull operations. GitHub grants this OAuth app repository write permission; GitMint uses it only for actions you start.</span>
  </section></main>

  return (
    <main className="window-shell">
      <aside className="sidebar">
        <div className="brand-row">
          <div className="brand-mark"><img src="/gitmint.svg" alt="" /></div>
          <span className="brand-name">GitMint</span>
          <span className="brand-version">1.0</span>
        </div>

        <div className="repo-picker">
          <span className="section-label">WORKSPACE</span>
          <button className="repo-select" onClick={() => openRepository()} disabled={busy}>
            <FolderGit2 size={17} />
            <span className="repo-select-text">{repository?.name ?? 'Select a repository'}</span>
            <ChevronDown className="repo-chevron" size={15} aria-hidden="true" />
          </button>
          {repository && <div className="repo-path" title={repository.path}>{repository.path}</div>}
        </div>

        <nav className="navigation" aria-label="Main navigation">
          <span className="section-label">WORKSPACE</span>
          <button className={`nav-item ${activeView === 'repositories' ? 'active' : 'muted'}`} onClick={() => { setActiveView('repositories'); setError(''); void refreshGithubRepositories() }}><Library size={16} /><span>Repositories</span></button>
          <span className="section-label nav-section-label">REPOSITORY</span>
          <button className={`nav-item ${activeView === 'history' ? 'active' : 'muted'}`} onClick={() => setActiveView('history')}><History size={16} /><span>History</span><kbd>⌘1</kbd></button>
          <button className={`nav-item ${activeView === 'branches' ? 'active' : 'muted'}`} onClick={() => { setActiveView('branches'); void refreshBranches() }} disabled={!repository}><GitBranch size={16} /><span>Branches</span></button>
          <button className={`nav-item ${activeView === 'changes' ? 'active' : 'muted'}`} onClick={() => { setActiveView('changes'); void refreshChanges() }} disabled={!repository}><GitCommitHorizontal size={16} /><span>Changes</span></button>
        </nav>

        <div className="sidebar-bottom">
          <div className="sidebar-footer"><button className={`nav-item ${activeView === 'settings' ? 'active' : 'muted'}`} onClick={() => setActiveView('settings')}><Settings2 size={16} /><span>Settings</span></button><span className="footer-mark">v1.1.0</span></div>
        </div>
      </aside>

      <section className="main-panel">
        <header className="topbar">
          <div className="breadcrumbs"><span>Workspace</span><span className="crumb-slash">/</span><strong>{activeView === 'settings' ? 'Settings' : activeView === 'repositories' ? 'Repositories' : repository?.name ?? 'No repository open'}</strong></div>
          <div className="topbar-actions">
            {repository && activeView !== 'repositories' && activeView !== 'settings' && <div className="branch-pill"><GitBranch size={13} /><span>{repository.branch}</span></div>}
            {repository && activeView !== 'repositories' && activeView !== 'settings' && <div className="sync-actions">
              <button className="sync-button" onClick={() => void syncRepository('pull')} disabled={syncBusy || busy || operationBusy} title="Pull fast-forward changes from GitHub"><ArrowDownToLine size={13} />Pull</button>
              <button className="sync-button" onClick={() => void syncRepository('push')} disabled={syncBusy || busy || operationBusy} title="Push this branch to GitHub"><ArrowUpFromLine size={13} />Push</button>
            </div>}
            {activeView !== 'repositories' && activeView !== 'settings' && <button className="icon-button" aria-label="Refresh repository" title="Refresh repository" onClick={() => repository && openRepository(repository.path)} disabled={!repository || busy}><RefreshCw size={15} className={busy ? 'spinning' : ''} /></button>}
            {githubUser && <button className="user-chip" onClick={() => void signOut()} title="Sign out of GitHub"><img src={githubUser.avatarUrl} alt="" /><span>{githubUser.login}</span><span className="signout-label">Sign out</span></button>}
          </div>
        </header>

        {error && <div className="error-banner" role="alert"><AlertCircle size={16} /><span>{error}</span><button onClick={() => setError('')} aria-label="Dismiss">×</button></div>}
        {notice && <div className="success-banner" role="status"><span className="success-mark">✓</span><span>{notice}</span><button onClick={() => setNotice('')} aria-label="Dismiss">×</button></div>}

        {activeView === 'settings' ? <div className="settings-view">
          <div className="repo-heading"><div><div className="eyebrow"><span className="status-dot" /> PREFERENCES</div><h1>Settings</h1><p className="repo-subtitle">Choose how your repository library opens.</p></div></div>
          <section className="settings-section"><div className="settings-section-title"><h2>Repository library</h2><p>Choose which repositories you see first when you open the library.</p></div><div className="settings-row settings-library-row"><div><strong>Open to</strong><span>You can switch sources anytime in Repositories.</span></div><div className="settings-choice" role="group" aria-label="Default repository library"><button type="button" className={libraryTab === 'github' ? 'selected' : ''} aria-pressed={libraryTab === 'github'} onClick={() => setLibraryTab('github')}><GithubMark size={14}/>GitHub</button><button type="button" className={libraryTab === 'local' ? 'selected' : ''} aria-pressed={libraryTab === 'local'} onClick={() => setLibraryTab('local')}><FolderGit2 size={14}/>On this Mac</button></div></div></section>
          <section className="settings-section"><div className="settings-section-title"><h2>GitHub account</h2><p>Signed in as {githubUser.login}. GitMint uses GitHub for repository browsing and sync actions you start.</p></div><div className="settings-row"><div className="settings-account"><img src={githubUser.avatarUrl} alt=""/><div><strong>{githubUser.login}</strong><span>Connected</span></div></div><button className="settings-action secondary" onClick={() => void signOut()}>Sign out</button></div></section>
          <section className="settings-section"><div className="settings-section-title"><h2>Recent repositories</h2><p>Clear GitMint’s remembered list. This never deletes repository files from your Mac.</p></div><div className="settings-row"><div><strong>{recent.length} {recent.length === 1 ? 'repository' : 'repositories'}</strong><span>Saved on this Mac</span></div><button className="settings-action" disabled={recent.length === 0} onClick={() => { if (window.confirm('Clear the recently opened repository list? Your repository files will not be deleted.')) { setRecent([]); setNotice('Recent repositories cleared') } }}>Clear list</button></div></section>
        </div> : activeView === 'repositories' && selectedGithubRepository ? <GithubRepositoryDetail
          item={selectedGithubRepository}
          readme={repositoryReadme}
          readmeError={repositoryReadmeError}
          loading={repositoryReadmeLoading}
          cloning={cloningRepository === selectedGithubRepository.id}
          onBack={() => { setSelectedGithubRepository(null); setRepositoryReadme(null) }}
          onCopy={() => void copyRepositoryUrl(selectedGithubRepository.htmlUrl)}
          onClone={() => void cloneGithubRepository(selectedGithubRepository)}
          onOpen={() => void invoke('open_github_repository', { url: selectedGithubRepository.htmlUrl }).catch((reason: unknown) => setError(String(reason)))}
        /> : activeView === 'repositories' ? <div className="library-view">
          <div className="repo-heading library-heading">
            <div><div className="eyebrow"><span className="status-dot" /> YOUR WORKSPACE</div><h1>Repositories</h1><p className="repo-subtitle">Browse your repositories, search GitHub, or open a project on this Mac.</p></div>
            <button className="primary-button library-add-button" onClick={() => { setLibraryTab('local'); void openRepository() }} disabled={busy}><FolderOpen size={15} />Add local repository</button>
          </div>
          <div className="library-toolbar">
            <div className="library-tabs" role="tablist" aria-label="Repository sources">
              <button className={libraryTab === 'github' ? 'selected' : ''} onClick={() => { setSelectedGithubRepository(null); if (libraryTab !== 'github') { setLibraryTab('github'); setGithubSearchResults(null); setGithubSearchLoading(false); setGithubSearchFailed(false) } }}><GithubMark size={14} />GitHub <span>{githubRepositories.length}</span></button>
              <button className={libraryTab === 'local' ? 'selected' : ''} onClick={() => { setSelectedGithubRepository(null); if (libraryTab !== 'local') { setLibraryTab('local'); setGithubSearchResults(null); setGithubSearchLoading(false); setGithubSearchFailed(false) } }}><FolderGit2 size={14} />On this Mac <span>{recent.length}</span></button>
            </div>
            <label className="repository-search"><Search size={14} /><input value={repositorySearch} onChange={(event) => { setRepositorySearch(event.target.value); setGithubSearchResults(null); setGithubSearchLoading(false); setGithubSearchFailed(false) }} placeholder={libraryTab === 'github' ? 'Search all GitHub repositories' : 'Find a local repository'} aria-label={libraryTab === 'github' ? 'Search all GitHub repositories' : 'Search local repositories'} /></label>
            {libraryTab === 'github' && <button className="icon-button library-refresh" aria-label="Refresh GitHub repositories" title="Refresh GitHub repositories" onClick={() => void refreshGithubRepositories()} disabled={githubRepositoriesLoading}><RefreshCw size={14} className={githubRepositoriesLoading ? 'spinning' : ''} /></button>}
          </div>
          {libraryTab === 'github' ? <div className="repository-catalog" aria-label="GitHub repositories">
            {globalSearchActive ? githubSearchLoading || (!githubSearchResults && !githubSearchFailed) ? <div className="catalog-empty"><div className="auth-spinner" /><strong>Searching GitHub…</strong><span>Looking across public repositories.</span></div> : githubSearchFailed ? <div className="catalog-empty"><GithubMark size={23} /><strong>Search didn’t finish</strong><span>Check your connection and try again.</span><button className="load-more-button" onClick={() => void searchGithubRepositories(repositorySearch.trim())}>Try again</button></div> : displayedGithubRepositories.length === 0 ? <div className="catalog-empty"><GithubMark size={23} /><strong>No repositories found</strong><span>Try a different name, topic, or phrase.</span></div> : <>
              <div className="catalog-search-summary">{githubSearchResults!.totalCount.toLocaleString()} repositories on GitHub match “{repositorySearch.trim()}”.</div>
              {displayedGithubRepositories.map((item) => <GithubRepositoryRow key={item.id} item={item} cloning={cloningRepository === item.id} disabled={cloningRepository !== null} onDetails={() => void showGithubRepository(item)} onClone={() => void cloneGithubRepository(item)} onCopy={() => void copyRepositoryUrl(item.htmlUrl)} onOpen={() => void invoke('open_github_repository', { url: item.htmlUrl }).catch((reason: unknown) => setError(String(reason)))} />)}
              {githubSearchResults!.items.length < githubSearchResults!.totalCount && githubSearchResults!.page < 20 && <button className="load-more-button" onClick={() => void searchGithubRepositories(repositorySearch.trim(), githubSearchResults!.page + 1, true)} disabled={githubSearchLoading}>{githubSearchLoading ? 'Loading…' : 'Load more results'}</button>}
            </> : githubRepositoriesLoading && githubRepositories.length === 0 ? <div className="catalog-empty"><div className="auth-spinner" /><strong>Loading your GitHub repositories…</strong></div> : displayedGithubRepositories.length === 0 ? <div className="catalog-empty"><GithubMark size={23} /><strong>{repositorySearch ? 'No matching repositories' : 'No GitHub repositories yet'}</strong><span>{repositorySearch ? 'Enter at least 2 characters to search all of GitHub.' : 'Repositories you own or have access to will show up here. Search above to discover public repositories.'}</span></div> : displayedGithubRepositories.map((item) => <GithubRepositoryRow key={item.id} item={item} cloning={cloningRepository === item.id} disabled={cloningRepository !== null} onDetails={() => void showGithubRepository(item)} onClone={() => void cloneGithubRepository(item)} onCopy={() => void copyRepositoryUrl(item.htmlUrl)} onOpen={() => void invoke('open_github_repository', { url: item.htmlUrl }).catch((reason: unknown) => setError(String(reason)))} />)}
          </div> : <div className="repository-catalog" aria-label="Local repositories">
            {recent.filter((item) => `${item.name} ${item.path}`.toLowerCase().includes(repositorySearch.toLowerCase())).length === 0 ? <div className="catalog-empty"><FolderOpen size={23} /><strong>{repositorySearch ? 'No matching repositories' : 'No local repositories added'}</strong><span>{repositorySearch ? 'Try another name or path.' : 'Add a Git repository from your Mac to see it in your library.'}</span><button className="primary-button" onClick={() => void openRepository()} disabled={busy}><FolderOpen size={15} />Choose a repository</button></div> : recent.filter((item) => `${item.name} ${item.path}`.toLowerCase().includes(repositorySearch.toLowerCase())).map((item) => <button className={`catalog-row local-catalog-row ${repository?.path === item.path ? 'current' : ''}`} key={item.path} onClick={() => void openRepository(item.path)}>
              <div className="catalog-repo-icon"><FolderGit2 size={16} /></div><div className="catalog-repo-copy"><div className="catalog-repo-title"><strong>{item.name}</strong>{repository?.path === item.path && <span className="visibility-tag">Open</span>}</div><p className="local-repo-path">{item.path}</p><span className="catalog-repo-meta">Branch: {item.branch}</span></div><span className="local-open-label">Open <ChevronRight size={14} /></span>
            </button>)}
          </div>}
        </div> : repository ? <div className="repository-view">
          <div className="repo-heading">
            <div><div className="eyebrow"><span className="status-dot" /> REPOSITORY CONNECTED</div><h1>{repository.name}</h1><p className="repo-subtitle">Ready when you are. Your repository is open.</p></div>
            <div className="heading-branch"><GitBranch size={15} /><span>{repository.branch}</span></div>
          </div>
          {activeView === 'history' ? <div className="history-layout">
            <section className="commit-list-panel" aria-label="Commit history">
              <div className="panel-heading"><span>COMMITS</span><span className="commit-count">{commits.length}{commits.length === 100 ? '+' : ''}</span></div>
              {loadingHistory ? <div className="history-message">Loading history…</div> : commits.length === 0 ? <div className="history-message empty-history"><div className="empty-glyph"><History size={22} /></div><strong>No commits yet</strong><span>Make your first commit to start building a history.</span></div> : <div className="commit-list">
                {commits.map((commit) => <button key={commit.hash} className={`commit-row ${selectedCommit === commit.hash ? 'selected' : ''}`} onClick={() => setSelectedCommit(commit.hash)}>
                  <span className="commit-marker"><GitCommitHorizontal size={15} /></span><span className="commit-row-copy"><strong>{commit.message}</strong><span>{commit.author} <i>·</i> {formatDate(commit.authoredAt)}</span></span><code>{commit.shortHash}</code>
                </button>)}
              </div>}
            </section>
            <section className="commit-details-panel" aria-label="Selected commit details">
              {commitDetails?.hash === selectedCommit ? <>
                <div className="detail-title"><span className="detail-label">COMMIT DETAILS</span><h2>{commitDetails.message.split('\n')[0]}</h2><p>{commitDetails.author} <span>·</span> {formatDate(commitDetails.authoredAt)}</p><code>{commitDetails.hash}</code></div>
                {commitDetails.message.split('\n').slice(1).join('\n').trim() && <p className="commit-body">{commitDetails.message.split('\n').slice(1).join('\n').trim()}</p>}
                <div className="changed-files-heading"><span>FILES CHANGED</span><span>{commitDetails.files.length}</span></div>
                {commitDetails.files.length ? <div className="changed-file-list">{commitDetails.files.map((file) => <div className="changed-file" key={file}><FileCode2 size={14} /><span>{file}</span><ChevronRight size={13} /></div>)}</div> : <div className="history-message">No files changed in this commit.</div>}
              </> : <div className="detail-placeholder"><History size={21} /><span>{commits.length ? 'Select a commit to inspect its details.' : 'Commit details will appear here.'}</span></div>}
            </section>
          </div> : activeView === 'changes' ? <><div className="changes-layout">
            <section className="changes-list-panel" aria-label="Working tree changes">
              <div className="changes-panel-header"><span>WORKING TREE</span><div className="changes-panel-actions">{changes.some((file) => file.unstagedStatus) && <button className="stage-all-button" onClick={() => void stageAllChanges()} disabled={operationBusy}><Plus size={12} />Stage all</button>}<button onClick={() => void refreshChanges()} title="Refresh changes" aria-label="Refresh changes"><RefreshCw size={13} /></button></div></div>
              {loadingChanges ? <div className="history-message">Refreshing working tree…</div> : changes.length === 0 ? <div className="clean-state"><div className="clean-check">✓</div><strong>Working tree clean</strong><span>No staged or unstaged changes.</span></div> : <>
                <ChangeGroup title="STAGED CHANGES" files={changes.filter((file) => file.stagedStatus)} staged selected={selectedFile} busy={operationBusy} onSelect={(file) => void selectFile(file.path, true)} onAction={(file) => void stageFile(file, true)} />
                <ChangeGroup title="UNSTAGED CHANGES" files={changes.filter((file) => file.unstagedStatus)} staged={false} selected={selectedFile} busy={operationBusy} onSelect={(file) => void selectFile(file.path, false)} onAction={(file) => void stageFile(file, false)} />
              </>}
            </section>
            <section className="diff-panel" aria-label="Selected file diff">
              {selectedFile ? <>
                <div className="diff-header"><div><FileCode2 size={15} /><strong>{selectedFile.path}</strong><span className="diff-state">{selectedFile.staged ? 'STAGED' : 'UNSTAGED'}</span></div><span>{fileDiff?.text.split('\n').length ?? '—'} lines</span></div>
                {loadingDiff ? <div className="detail-placeholder">Loading diff…</div> : fileDiff?.text ? <DiffViewer text={fileDiff.text} /> : <div className="detail-placeholder">No text diff is available for this file.</div>}
              </> : <div className="detail-placeholder"><GitCommitHorizontal size={22} /><span>Select a changed file to inspect its diff.</span></div>}
            </section>
          </div><form className="commit-composer" onSubmit={commitChanges}><div className="commit-composer-label"><span className="status-dot" />{changes.filter((file) => file.stagedStatus).length} staged</div><input value={commitMessage} onChange={(event) => setCommitMessage(event.target.value)} placeholder="Commit message…" aria-label="Commit message" /><button type="submit" disabled={operationBusy || !commitMessage.trim() || !changes.some((file) => file.stagedStatus)}><GitCommitHorizontal size={14} />Commit</button></form></> : <div className="branch-view">
            <div className="branch-view-heading"><div><span className="detail-label">LOCAL BRANCHES</span><h2>Keep work moving</h2><p>Create a branch or switch to a clean working branch.</p></div><form className="new-branch-form" onSubmit={createBranch}><input value={branchName} onChange={(event) => setBranchName(event.target.value)} placeholder="New branch name" aria-label="New branch name" /><button type="submit" disabled={operationBusy || !branchName.trim() || branches.length === 0}><Plus size={14} />Create branch</button></form></div>
            <div className="branch-list">{branches.length ? branches.map((branch) => <div className={`branch-row ${branch.current ? 'current' : ''}`} key={branch.name}><div className="branch-row-main"><GitBranch size={16} /><span>{branch.name}</span>{branch.current && <span className="current-branch-tag">CURRENT</span>}</div>{!branch.current && <button onClick={() => void switchBranch(branch.name)} disabled={operationBusy}>Switch</button>}</div>) : <div className="branch-empty"><GitBranch size={22} /><strong>No local branches yet</strong><span>Create a first commit to create your starting branch.</span></div>}</div>
          </div>}
          <div className="repo-location"><FolderOpen size={14} /><span>{repository.path}</span></div>
        </div> : <div className="welcome-wrap">
          <div className="welcome-content">
            <div className="welcome-icon"><div className="mint-orb"><Leaf size={28} strokeWidth={1.6} /></div><span className="orbit orbit-one" /><span className="orbit orbit-two" /></div>
            <div className="welcome-kicker"><Sparkles size={12} /> A FRESH TAKE ON GIT</div>
            <h1>Your Git workflow,<br /><em>refreshed.</em></h1>
            <p className="welcome-copy">A calmer way to work with Git.<br />Pick a repository and let’s get started.</p>
            <button className="primary-button" onClick={() => openRepository()} disabled={busy}><FolderOpen size={16} />{busy ? 'Opening…' : 'Open a repository'}<span className="shortcut">⌘ O</span></button>
            <div className="path-entry"><span>or enter a local path</span><form onSubmit={(event) => { event.preventDefault(); if (pathInput.trim()) void openRepository(pathInput.trim()) }}><input value={pathInput} onChange={(event) => setPathInput(event.target.value)} placeholder="/Users/you/projects/my-app" aria-label="Repository path" /><button type="submit" disabled={!pathInput.trim() || busy} aria-label="Open path">↗</button></form></div>
          </div>
          <div className="welcome-footer"><span><span className="footer-dot" /> LOCAL FIRST · YOUR CODE STAYS YOURS</span><span>Built for the way you work.</span></div>
        </div>}
      </section>
    </main>
  )
}

function ChangeGroup({ title, files, staged, selected, busy, onSelect, onAction }: { title: string; files: ChangedFile[]; staged: boolean; selected: SelectedFile | null; busy: boolean; onSelect: (file: ChangedFile) => void; onAction: (file: ChangedFile) => void }) {
  return <div className="change-group">
    <div className="change-group-heading"><span>{title}</span><span>{files.length}</span></div>
    {files.length ? files.map((file) => {
      const status = staged ? file.stagedStatus : file.unstagedStatus
      return <div className={`change-file-row ${selected?.path === file.path && selected?.staged === staged ? 'selected' : ''}`} key={`${title}:${file.path}`}>
        <button className="change-file-main" onClick={() => onSelect(file)} title={file.path}><FileCode2 size={14} /><span className="change-file-name">{file.path}</span><span className={`status-badge status-${status?.toLowerCase()}`}>{status}</span></button>
        <button className="file-action-button" onClick={() => onAction(file)} disabled={busy} aria-label={staged ? `Unstage ${file.path}` : `Stage ${file.path}`} title={staged ? 'Unstage file' : 'Stage file'}>{staged ? <Minus size={14} /> : <Plus size={14} />}</button>
      </div>
    }) : <div className="change-group-empty">No files</div>}
  </div>
}

function GithubRepositoryRow({ item, cloning, disabled, onDetails, onClone, onCopy, onOpen }: {
  item: GithubRepository
  cloning: boolean
  disabled: boolean
  onDetails: () => void
  onClone: () => void
  onCopy: () => void
  onOpen: () => void
}) {
  return <div className="catalog-row github-catalog-row">
    <div className="catalog-repo-icon"><FolderGit2 size={16} /></div>
    <button className="catalog-repo-copy catalog-details-link" onClick={onDetails} aria-label={`View details for ${item.fullName}`}>
      <span className="catalog-repo-title"><strong>{item.fullName}</strong>{item.private && <span className="visibility-tag"><LockKeyhole size={10} />Private</span>}{item.fork && <span className="visibility-tag">Fork</span>}</span>
      <span className="catalog-description">{item.description || 'No description provided.'}</span>
      <span className="catalog-repo-meta">Updated {formatDate(item.updatedAt)} <i>·</i> {item.defaultBranch}</span>
    </button>
    <div className="github-repo-actions">
      <button className="catalog-open-button" onClick={onClone} disabled={disabled}>{cloning ? <RefreshCw size={13} className="spinning" /> : <Download size={13} />}{cloning ? 'Cloning…' : 'Clone & open'}</button>
      <button className="icon-button" aria-label={`Copy URL for ${item.fullName}`} title="Copy repository URL" onClick={onCopy}><Copy size={13} /></button>
      <button className="icon-button" aria-label={`Open ${item.fullName} on GitHub`} title="Open on GitHub" onClick={onOpen}><ExternalLink size={13} /></button>
    </div>
  </div>
}

function GithubRepositoryDetail({ item, readme, readmeError, loading, cloning, onBack, onCopy, onClone, onOpen }: {
  item: GithubRepository
  readme: string | null
  readmeError: string
  loading: boolean
  cloning: boolean
  onBack: () => void
  onCopy: () => void
  onClone: () => void
  onOpen: () => void
}) {
  return <div className="library-view github-detail-view">
    <button className="detail-back-button" onClick={onBack}><ChevronLeft size={15} />Back to repositories</button>
    <div className="github-detail-heading">
      <div className="github-detail-title"><div className="catalog-repo-icon"><FolderGit2 size={18} /></div><div><div className="catalog-repo-title"><h1>{item.fullName}</h1>{item.private && <span className="visibility-tag"><LockKeyhole size={10} />Private</span>}{item.fork && <span className="visibility-tag">Fork</span>}</div><p>{item.description || 'No description provided.'}</p></div></div>
      <div className="github-detail-actions">
        <button className="catalog-open-button" onClick={onClone} disabled={cloning}>{cloning ? <RefreshCw size={13} className="spinning" /> : <Download size={13} />}{cloning ? 'Cloning…' : 'Clone & open'}</button>
        <button className="detail-action-button" onClick={onCopy}><Copy size={13} />Copy URL</button>
        <button className="detail-action-button" onClick={onOpen}><ExternalLink size={13} />Open on GitHub</button>
      </div>
    </div>
    <div className="github-detail-meta"><span>Default branch <strong>{item.defaultBranch}</strong></span><span>Updated <strong>{formatDate(item.updatedAt)}</strong></span></div>
    <section className="readme-panel" aria-label={`${item.fullName} README`}>
      <div className="readme-panel-heading"><strong>README.md</strong><span>{item.fullName}</span></div>
      {loading ? <div className="readme-state"><div className="auth-spinner" /><span>Loading README…</span></div> : readme ? <article className="readme-content">
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={[rehypeRaw, [rehypeSanitize, { ...defaultSchema, tagNames: [...(defaultSchema.tagNames ?? []), 'picture', 'source'], attributes: { ...defaultSchema.attributes, '*': [...(defaultSchema.attributes?.['*'] ?? []), 'align'], picture: [], source: ['media', 'srcSet', 'type'] } }] as never]}
          urlTransform={(url, key) => githubReadmeUrl(url, key, item)}
          components={{ a: ({ ...props }) => <a {...props} target="_blank" rel="noreferrer" /> }}
        >{readme}</ReactMarkdown>
      </article> : readmeError ? <div className="readme-state"><strong>Couldn’t load the README</strong><span>{readmeError}</span></div> : <div className="readme-state"><strong>No README found</strong><span>This repository doesn’t include a README file.</span></div>}
    </section>
  </div>
}

function githubReadmeUrl(url: string, key: string, item: GithubRepository) {
  if (/^(?:[a-z][a-z\d+.-]*:|\/\/|#)/i.test(url)) return url
  const [path, suffix = ''] = url.split(/(?=[?#])/, 2)
  const encodedPath = path.split('/').map(encodeURIComponent).join('/')
  const branch = encodeURIComponent(item.defaultBranch)
  const base = key === 'src' || key === 'srcSet'
    ? `https://raw.githubusercontent.com/${item.fullName}/${branch}/${encodedPath}`
    : `https://github.com/${item.fullName}/blob/${branch}/${encodedPath}`
  return `${base}${suffix}`
}

function Brand() {
  return <div className="auth-brand"><span className="auth-brand-mark"><img src="/gitmint.svg" alt="" /></span><strong>GitMint</strong><span>1.0</span></div>
}

function GithubMark({ size }: { size: number }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 .8a11.2 11.2 0 0 0-3.54 21.83c.56.1.77-.24.77-.54v-2.1c-3.14.68-3.8-1.33-3.8-1.33-.5-1.3-1.25-1.65-1.25-1.65-1.02-.7.08-.69.08-.69 1.13.08 1.73 1.17 1.73 1.17 1 .1.77 2.48 3.48 1.77.1-.73.4-1.23.7-1.52-2.5-.29-5.13-1.25-5.13-5.57 0-1.23.44-2.23 1.17-3.02-.12-.29-.51-1.43.11-2.98 0 0 .95-.3 3.08 1.15a10.7 10.7 0 0 1 5.6 0c2.13-1.45 3.08-1.15 3.08-1.15.62 1.55.23 2.69.11 2.98.73.8 1.17 1.8 1.17 3.02 0 4.33-2.63 5.28-5.14 5.56.41.35.77 1.03.77 2.08v3.08c0 .3.2.65.78.54A11.2 11.2 0 0 0 12 .8Z" /></svg>
}

function DiffViewer({ text }: { text: string }) {
  return <div className="diff-content">{text.replace(/\n$/, '').split('\n').map((line, index) => {
    const kind = line.startsWith('@@') ? 'hunk' : line.startsWith('+') && !line.startsWith('+++') ? 'added' : line.startsWith('-') && !line.startsWith('---') ? 'removed' : /^(diff --git|index |--- |\+\+\+)/.test(line) ? 'meta' : 'context'
    return <div className={`diff-line diff-${kind}`} key={`${index}:${line}`}><span className="line-number">{index + 1}</span><code>{line || ' '}</code></div>
  })}</div>
}

function formatDate(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', year: 'numeric' }).format(date)
}

export default App
