const decisionStatuses = ['keep', 'fix', 'review', 'archive']
const state = {
  site: null,
  selectedId: null,
  search: '',
  group: 'all',
  risk: 'all',
  status: 'all'
}

const $ = (selector) => document.querySelector(selector)

function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>"']/g, (char) => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;'
  })[char])
}

function repoHref(filePath) {
  return `/repo/${String(filePath).split('/').map(encodeURIComponent).join('/')}`
}

function sourceLinks(paths = []) {
  return paths.map((filePath) => (
    `<a class="path" href="${repoHref(filePath)}" target="_blank" rel="noreferrer">${escapeHtml(filePath)}</a>`
  )).join('')
}

function decisionFor(object) {
  return state.site.decisions.find((entry) => entry.objectId === object.id)?.status
    || object.status
    || object.recommendation
    || 'review'
}

function unique(values) {
  return [...new Set(values.filter(Boolean))].sort((a, b) => a.localeCompare(b))
}

function optionList(id, values, label) {
  const select = $(id)
  const current = select.value || 'all'
  select.innerHTML = [
    `<option value="all">${label}</option>`,
    ...values.map((value) => `<option value="${escapeHtml(value)}">${escapeHtml(value)}</option>`)
  ].join('')
  select.value = values.includes(current) ? current : 'all'
}

function searchableText(object) {
  return [
    object.id,
    object.title,
    object.summary,
    object.evidence,
    object.group,
    object.kind,
    object.risk,
    object.status,
    ...(object.paths || []),
    ...(object.tags || []),
    ...(object.flags || [])
  ].join(' ').toLowerCase()
}

function filteredObjects() {
  return state.site.objects.filter((object) => {
    const status = decisionFor(object)
    const searchOk = !state.search || searchableText(object).includes(state.search)
    const groupOk = state.group === 'all' || object.group === state.group
    const riskOk = state.risk === 'all' || object.risk === state.risk
    const statusOk = state.status === 'all' || status === state.status
    return searchOk && groupOk && riskOk && statusOk
  })
}

function renderFilters() {
  optionList('#group-filter', unique(state.site.objects.map((object) => object.group)), 'all groups')
  optionList('#risk-filter', unique(state.site.objects.map((object) => object.risk)), 'all risks')
  optionList('#status-filter', unique(decisionStatuses), 'all states')
}

function renderMetrics(objects) {
  const highRisk = state.site.objects.filter((object) => ['high', 'critical'].includes(object.risk)).length
  const staleDocs = state.site.objects.filter((object) => object.tags?.includes('stale-docs')).length
  const active = state.site.objects.filter((object) => object.status === 'keep').length

  const metrics = [
    { label: 'inventory cards', value: state.site.objects.length, detail: `${objects.length} visible` },
    { label: 'runtime flows', value: state.site.flows.length, detail: 'hotkey, rewrite, release' },
    { label: 'high risk', value: highRisk, detail: 'docs or release drift' },
    { label: 'active surfaces', value: active, detail: 'live codepaths' },
    { label: 'stale docs', value: staleDocs, detail: 'needs cleanup' },
    { label: 'actions', value: state.site.actions.length, detail: 'curated queue' },
    { label: 'comments', value: state.site.comments.length, detail: 'persisted notes' },
    { label: 'decisions', value: state.site.decisions.length, detail: 'persisted states' }
  ]

  $('#metrics').innerHTML = metrics.map((metric) => `
    <article class="metric">
      <b>${escapeHtml(metric.value)}</b>
      <span>${escapeHtml(metric.label)}</span>
      <p class="muted">${escapeHtml(metric.detail)}</p>
    </article>
  `).join('')
}

function renderPages() {
  $('#pages').innerHTML = state.site.pages.map((page) => `
    <article class="card ${escapeHtml(page.status || 'review')}" data-id="${escapeHtml(page.objects?.[0] || '')}">
      <div class="tag-row">
        <span class="tag">${escapeHtml(page.group)}</span>
        <span class="pill ${escapeHtml(page.status)}">${escapeHtml(page.status)}</span>
      </div>
      <h4>${escapeHtml(page.title)}</h4>
      <p>${escapeHtml(page.summary)}</p>
      <div class="path-list">
        ${sourceLinks(page.links?.map((link) => link.path) || [])}
      </div>
    </article>
  `).join('')
}

function renderFlows() {
  $('#flow-list').innerHTML = state.site.flows.map((flow) => `
    <article class="flow ${escapeHtml(flow.status)} ${escapeHtml(flow.risk)}">
      <div class="flow-header">
        <div>
          <div class="tag-row">
            <span class="tag">${escapeHtml(flow.group)}</span>
            <span class="pill ${escapeHtml(flow.risk)}">${escapeHtml(flow.risk)}</span>
          </div>
          <h4>${escapeHtml(flow.title)}</h4>
          <p>${escapeHtml(flow.summary)}</p>
        </div>
        <button data-flow-open="${escapeHtml(flow.related?.[0] || '')}">open</button>
      </div>
      <div class="flow-steps">
        ${flow.steps.map((step, index) => `
          <div class="flow-step">
            <span class="meta">0${index + 1}</span>
            <b>${escapeHtml(step.label)}</b>
            <p>${escapeHtml(step.summary)}</p>
          </div>
        `).join('')}
      </div>
    </article>
  `).join('')
}

function renderItems(objects) {
  $('#items').innerHTML = objects.map((object) => {
    const status = decisionFor(object)
    const commentCount = state.site.comments.filter((entry) => entry.objectId === object.id).length
    return `
      <article class="card ${escapeHtml(status)} ${escapeHtml(object.risk)}" data-id="${escapeHtml(object.id)}">
        <div class="tag-row">
          <span class="tag">${escapeHtml(object.group)}</span>
          <span class="pill ${escapeHtml(object.risk)}">${escapeHtml(object.risk)}</span>
          <span class="pill">${escapeHtml(status)}</span>
        </div>
        <h4>${escapeHtml(object.title)}</h4>
        <p>${escapeHtml(object.summary)}</p>
        <div class="path-list">${sourceLinks((object.paths || []).slice(0, 3))}</div>
        <span class="meta">${commentCount} notes</span>
      </article>
    `
  }).join('') || '<div class="empty">no cards match the filters</div>'
}

function renderQueue() {
  $('#queue').innerHTML = state.site.actions.map((action) => `
    <article class="queue-item ${escapeHtml(action.status)} ${escapeHtml(action.priority)}">
      <div class="tag-row">
        <span class="tag">${escapeHtml(action.priority)}</span>
        <span class="pill">${escapeHtml(action.status)}</span>
      </div>
      <h4>${escapeHtml(action.title)}</h4>
      <p>${escapeHtml(action.summary)}</p>
      <div class="path-list">${sourceLinks(action.paths || [])}</div>
    </article>
  `).join('')
}

function renderDetail() {
  const object = state.site.objects.find((entry) => entry.id === state.selectedId)
  if (!object) {
    $('#detail').innerHTML = `
      <div class="detail-empty">
        <span class="eyebrow">detail</span>
        <p>pick an inventory card to see source files, risk, tests, and notes.</p>
      </div>
    `
    return
  }

  const status = decisionFor(object)
  const comments = state.site.comments.filter((entry) => entry.objectId === object.id)
  const buttons = decisionStatuses.map((entry) => (
    `<button class="${entry} ${entry === status ? 'active' : ''}" data-decision="${entry}" data-id="${object.id}">${entry}</button>`
  )).join('')

  $('#detail').innerHTML = `
    <div class="detail-block">
      <span class="eyebrow">${escapeHtml(object.group)}</span>
      <h4>${escapeHtml(object.title)}</h4>
      <p>${escapeHtml(object.summary)}</p>
      <div class="tag-row">
        <span class="pill ${escapeHtml(object.risk)}">${escapeHtml(object.risk)}</span>
        <span class="pill">${escapeHtml(object.kind)}</span>
        <span class="pill">${escapeHtml(status)}</span>
      </div>
    </div>

    <div class="detail-block">
      <span class="eyebrow">recommendation</span>
      <p>${escapeHtml(object.recommendation || 'review')}</p>
      <p>${escapeHtml(object.evidence || '')}</p>
    </div>

    <div class="detail-block">
      <span class="eyebrow">sources</span>
      <div class="path-list">${sourceLinks(object.paths || [])}</div>
    </div>

    <div class="detail-block">
      <span class="eyebrow">flags</span>
      <div class="tag-row">${(object.flags || []).map((flag) => `<span class="pill">${escapeHtml(flag)}</span>`).join('')}</div>
    </div>

    <div class="detail-block">
      <span class="eyebrow">tests</span>
      <div class="tag-row">${(object.tests || []).map((test) => `<span class="pill">${escapeHtml(test)}</span>`).join('') || '<span class="muted">no direct test listed</span>'}</div>
    </div>

    <div class="detail-block">
      <span class="eyebrow">state</span>
      <div class="button-row">${buttons}</div>
    </div>

    <div class="detail-block">
      <span class="eyebrow">notes</span>
      <textarea data-note="${escapeHtml(object.id)}" placeholder="leave a note for this card"></textarea>
      <button data-comment="${escapeHtml(object.id)}">add note</button>
      ${comments.map((comment) => `<div class="comment">${escapeHtml(comment.body)}<br><span class="meta">${escapeHtml(comment.createdAt)}</span></div>`).join('')}
    </div>
  `
}

function render() {
  const objects = filteredObjects()
  $('#site-title').textContent = state.site.meta.title
  $('#site-subtitle').textContent = state.site.meta.subtitle
  $('#nav-status').innerHTML = `
    active workspace: ${escapeHtml(state.site.meta.repository.activeWorkspace)}<br>
    branch: ${escapeHtml(state.site.meta.repository.branch)}<br>
    generated: ${escapeHtml(state.site.meta.generatedAt)}
  `
  renderMetrics(objects)
  renderPages()
  renderFlows()
  renderItems(objects)
  renderQueue()
  renderDetail()
}

async function load() {
  state.site = await fetch('/api/site').then((response) => response.json())
  state.selectedId = state.site.objects[0]?.id || null
  renderFilters()
  render()
}

document.addEventListener('input', (event) => {
  if (event.target.id === 'search') {
    state.search = event.target.value.trim().toLowerCase()
    render()
  }
})

document.addEventListener('change', (event) => {
  if (event.target.id === 'group-filter') state.group = event.target.value
  if (event.target.id === 'risk-filter') state.risk = event.target.value
  if (event.target.id === 'status-filter') state.status = event.target.value
  render()
})

document.addEventListener('click', async (event) => {
  const link = event.target.closest('a')
  if (link) return

  const flowOpen = event.target.closest('[data-flow-open]')
  if (flowOpen) {
    state.selectedId = flowOpen.dataset.flowOpen
    render()
    return
  }

  const decisionButton = event.target.closest('[data-decision]')
  if (decisionButton) {
    state.site = await fetch('/api/decision', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        objectId: decisionButton.dataset.id,
        status: decisionButton.dataset.decision,
        decision: decisionButton.dataset.decision,
        reason: 'marked in interactive doc site'
      })
    }).then((response) => response.json())
    render()
    return
  }

  const commentButton = event.target.closest('[data-comment]')
  if (commentButton) {
    const objectId = commentButton.dataset.comment
    const note = document.querySelector(`[data-note="${objectId}"]`)
    const body = note?.value.trim()
    if (!body) return
    state.site = await fetch('/api/comment', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ objectId, body, status: 'open' })
    }).then((response) => response.json())
    render()
    return
  }

  const card = event.target.closest('[data-id]')
  if (card?.dataset.id) {
    state.selectedId = card.dataset.id
    render()
  }
})

load()
