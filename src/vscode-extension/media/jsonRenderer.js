/**
 * Render a JSON tree, built from the raw body text, into a collapsible tree structure
 * @param {object} node - The JSON tree node to render
 * @param {HTMLElement} container - The container element
 */
function renderJSON(node, container) {
    if (node.type === 'string') {
        container.innerHTML += '<span class="json-string">"' + escapeHtml(node.value) + '"</span>';
    } else if (node.type === 'array') {
        renderCollection(node.items, '[', ']', container, (item, line) => renderJSON(item, line));
    } else if (node.type === 'object') {
        renderCollection(node.entries, '{', '}', container, ([key, value], line) => {
            line.innerHTML += '<span class="json-key">"' + escapeHtml(key) + '"</span>: ';
            renderJSON(value, line);
        });
    } else {
        container.innerHTML += '<span class="json-' + node.type + '">' + escapeHtml(node.text) + '</span>';
    }
}

function renderCollection(children, open, close, container, renderChild) {
    if (children.length === 0) {
        container.innerHTML += '<span class="json-bracket">' + open + close + '</span>';
        return;
    }

    const toggle = document.createElement('span');
    toggle.className = 'json-toggle';
    toggle.textContent = '▼';
    toggle.onclick = function(e) {
        e.stopPropagation();
        const content = this.nextElementSibling.nextElementSibling;
        const collapsed = content.style.display === 'none';
        content.style.display = collapsed ? 'block' : 'none';
        this.textContent = collapsed ? '▼' : '▶';
    };

    const bracket = document.createElement('span');
    bracket.className = 'json-bracket';
    bracket.textContent = open;

    const content = document.createElement('div');
    content.className = 'json-content';

    children.forEach((child, index) => {
        const line = document.createElement('div');
        line.className = 'json-line';
        renderChild(child, line);
        if (index < children.length - 1) {
            const comma = document.createElement('span');
            comma.className = 'json-comma';
            comma.textContent = ',';
            line.appendChild(comma);
        }
        content.appendChild(line);
    });

    const closeBracket = document.createElement('div');
    closeBracket.className = 'json-bracket';
    closeBracket.textContent = close;

    container.appendChild(toggle);
    container.appendChild(bracket);
    container.appendChild(content);
    container.appendChild(closeBracket);
}

function escapeHtml(text) {
    const map = {
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#039;'
    };
    return text.replace(/[&<>"']/g, m => map[m]);
}
