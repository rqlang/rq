function renderJSON(node, container) {
    if (typeof node === 'string') {
        container.innerHTML += '<span class="json-string">"' + escapeHtml(node) + '"</span>';
    } else if (node[0] === 'a') {
        renderCollection(node.slice(1), '[', ']', container, (item, line) => renderJSON(item, line));
    } else if (node[0] === 'o') {
        renderCollection(entriesOf(node), '{', '}', container, ([key, value], line) => {
            line.innerHTML += '<span class="json-key">"' + escapeHtml(key) + '"</span>: ';
            renderJSON(value, line);
        });
    } else {
        container.innerHTML += '<span class="' + literalClass(node[0]) + '">' + escapeHtml(node[0]) + '</span>';
    }
}

function entriesOf(node) {
    const entries = [];
    for (let index = 1; index < node.length; index += 2) {
        entries.push([node[index], node[index + 1]]);
    }
    return entries;
}

function literalClass(text) {
    if (text === 'null') {
        return 'json-null';
    }
    return text === 'true' || text === 'false' ? 'json-boolean' : 'json-number';
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
