import { createServer } from 'node:http'
import { readFile, readdir, writeFile } from 'node:fs/promises'
import { createReadStream, existsSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(root, '..')
const publicDir = path.join(root, 'public')
const sitePath = path.join(root, 'data', 'site.json')
const preferredPort = Number(process.env.PORT || 8799)
const host = process.env.HOST || '127.0.0.1'

function send(res, code, data, type = 'application/json; charset=utf-8') {
  res.writeHead(code, { 'Content-Type': type, 'Cache-Control': 'no-store' })
  res.end(typeof data === 'string' ? data : JSON.stringify(data, null, 2))
}

async function readSite() {
  return JSON.parse(await readFile(sitePath, 'utf8'))
}

async function saveSite(site) {
  await writeFile(sitePath, JSON.stringify(site, null, 2) + '\n')
}

async function bodyJson(req) {
  const chunks = []
  for await (const chunk of req) chunks.push(chunk)
  const raw = Buffer.concat(chunks).toString('utf8')
  return raw ? JSON.parse(raw) : {}
}

function repoFile(pathname) {
  const rel = decodeURIComponent(pathname.replace(/^\/repo\//, ''))
  const file = path.resolve(repoRoot, rel)
  if (!file.startsWith(repoRoot) || file.includes(`${path.sep}.git${path.sep}`)) {
    return null
  }
  return file
}

function typeFor(file) {
  if (file.endsWith('.html')) return 'text/html; charset=utf-8'
  if (file.endsWith('.css')) return 'text/css; charset=utf-8'
  if (file.endsWith('.js') || file.endsWith('.mjs')) return 'text/javascript; charset=utf-8'
  if (file.endsWith('.json')) return 'application/json; charset=utf-8'
  if (file.endsWith('.md')) return 'text/markdown; charset=utf-8'
  if (file.endsWith('.toml') || file.endsWith('.rs') || file.endsWith('.yml')) {
    return 'text/plain; charset=utf-8'
  }
  return 'text/plain; charset=utf-8'
}

async function handleRequest(req, res) {
  try {
    const url = new URL(req.url || '/', 'http://localhost')

    if (url.pathname === '/api/site' && req.method === 'GET') {
      return send(res, 200, await readSite())
    }

    if (url.pathname === '/api/comment' && req.method === 'POST') {
      const site = await readSite()
      const input = await bodyJson(req)
      const body = String(input.body || '').trim()
      if (!body) return send(res, 400, { error: 'comment body is required' })

      const now = new Date().toISOString()
      site.comments.push({
        id: input.id || `comment-${Date.now()}`,
        objectId: String(input.objectId || ''),
        body,
        status: input.status || 'open',
        createdAt: now,
        updatedAt: now
      })
      await saveSite(site)
      return send(res, 201, site)
    }

    if (url.pathname === '/api/decision' && req.method === 'POST') {
      const site = await readSite()
      const input = await bodyJson(req)
      const now = new Date().toISOString()
      const objectId = String(input.objectId || '')
      let decision = site.decisions.find((entry) => entry.objectId === objectId)

      if (!decision) {
        decision = {
          id: `decision-${objectId}`,
          objectId,
          status: 'review',
          decision: 'needs-review',
          reason: '',
          updatedAt: now
        }
        site.decisions.push(decision)
      }

      decision.status = input.status || decision.status
      decision.decision = input.decision || decision.decision
      decision.reason = input.reason || decision.reason
      decision.updatedAt = now
      await saveSite(site)
      return send(res, 200, site)
    }

    if (url.pathname.startsWith('/repo/') && req.method === 'GET') {
      const file = repoFile(url.pathname)
      if (!file || !existsSync(file)) {
        return send(res, 404, 'not found', 'text/plain; charset=utf-8')
      }
      const info = statSync(file)
      if (info.isDirectory()) {
        const entries = await readdir(file)
        return send(res, 200, entries.sort().join('\n'), 'text/plain; charset=utf-8')
      }
      if (!info.isFile()) return send(res, 404, 'not found', 'text/plain; charset=utf-8')
      res.writeHead(200, { 'Content-Type': typeFor(file), 'Cache-Control': 'no-store' })
      return createReadStream(file).pipe(res)
    }

    const cleanPath = url.pathname === '/' ? '/index.html' : url.pathname
    const file = path.normalize(path.join(publicDir, cleanPath))
    if (!file.startsWith(publicDir) || !existsSync(file)) {
      return send(res, 404, 'not found', 'text/plain; charset=utf-8')
    }

    res.writeHead(200, { 'Content-Type': typeFor(file), 'Cache-Control': 'no-store' })
    createReadStream(file).pipe(res)
  } catch (error) {
    send(res, 500, { error: error instanceof Error ? error.message : String(error) })
  }
}

function listen(port) {
  const server = createServer(handleRequest)

  server.once('error', (error) => {
    if (error?.code === 'EADDRINUSE' && !process.env.PORT && port < preferredPort + 20) {
      listen(port + 1)
      return
    }
    throw error
  })

  server.listen(port, host, () => {
    const displayHost = host === '0.0.0.0' ? '127.0.0.1' : host
    console.log(`interactive doc site running at http://${displayHost}:${port}`)
  })
}

listen(preferredPort)
