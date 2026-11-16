function renderJSON(root, container) {
    const tasks = [() => renderNode(root, container, tasks)];
    while (tasks.length > 0) {
        tasks.pop()();
    }
}

function renderNode(node, container, tasks) {
    if (typeof node === 'string') {
        container.innerHTML += '<span class="json-string">"' + escapeHtml(node) + '"</span>';
    } else if (node[0] === 'a') {
        renderCollection(node.slice(1).map(value => ({ value })), '[', ']', container, tasks);
    } else if (node[0] === 'o') {
        renderCollection(entriesOf(node), '{', '}', container, tasks);
    } else {
        container.innerHTML += '<span class="' + literalClass(node[0]) + '">' + escapeHtml(node[0]) + '</span>';
    }
}

function entriesOf(node) {
    const entries = [];
    for (let index = 1; index < node.length; index += 2) {
        entries.push({ key: node[index], value: node[index + 1] });
    }
    return entries;
}

function literalClass(text) {
    if (text === 'null') {
        return 'json-null';
    }
    return text === 'true' || text === 'false' ? 'json-boolean' : 'json-number';
}

function renderCollection(children, open, close, container, tasks) {
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

    const lines = children.map(({ key }) => {
        const line = document.createElement('div');
        line.className = 'json-line';
        if (key !== undefined) {
            line.innerHTML += '<span class="json-key">"' + escapeHtml(key) + '"</span>: ';
        }
        content.appendChild(line);
        return line;
    });

    for (let index = children.length - 1; index >= 0; index--) {
        if (index < children.length - 1) {
            tasks.push(() => appendComma(lines[index]));
        }
        tasks.push(() => renderNode(children[index].value, lines[index], tasks));
    }

    const closeBracket = document.createElement('div');
    closeBracket.className = 'json-bracket';
    closeBracket.textContent = close;

    container.appendChild(toggle);
    container.appendChild(bracket);
    container.appendChild(content);
    container.appendChild(closeBracket);
}

function appendComma(line) {
    const comma = document.createElement('span');
    comma.className = 'json-comma';
    comma.textContent = ',';
    line.appendChild(comma);
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
