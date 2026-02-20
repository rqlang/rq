/**
 * Render JSON data into a collapsible tree structure
 * @param {any} data - The JSON data to render
 * @param {HTMLElement} container - The container element
 */
function renderJSON(data, container) {
    if (data === null) {
        container.innerHTML += '<span class="json-null">null</span>';
    } else if (typeof data === 'boolean') {
        container.innerHTML += '<span class="json-boolean">' + data + '</span>';
    } else if (typeof data === 'number') {
        container.innerHTML += '<span class="json-number">' + data + '</span>';
    } else if (typeof data === 'string') {
        container.innerHTML += '<span class="json-string">"' + escapeHtml(data) + '"</span>';
    } else if (Array.isArray(data)) {
        if (data.length === 0) {
            container.innerHTML += '<span class="json-bracket">[]</span>';
        } else {
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
            bracket.textContent = '[';
            
            const content = document.createElement('div');
            content.className = 'json-content';
            
            data.forEach((item, index) => {
                const line = document.createElement('div');
                line.className = 'json-line';
                renderJSON(item, line);
                if (index < data.length - 1) {
                    const comma = document.createElement('span');
                    comma.className = 'json-comma';
                    comma.textContent = ',';
                    line.appendChild(comma);
                }
                content.appendChild(line);
            });
            
            const closeBracket = document.createElement('div');
            closeBracket.className = 'json-bracket';
            closeBracket.textContent = ']';
            
            container.appendChild(toggle);
            container.appendChild(bracket);
            container.appendChild(content);
            container.appendChild(closeBracket);
        }
    } else if (typeof data === 'object') {
        const keys = Object.keys(data);
        if (keys.length === 0) {
            container.innerHTML += '<span class="json-bracket">{}</span>';
        } else {
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
            bracket.textContent = '{';
            
            const content = document.createElement('div');
            content.className = 'json-content';
            
            keys.forEach((key, index) => {
                const line = document.createElement('div');
                line.className = 'json-line';
                
                line.innerHTML += '<span class="json-key">"' + key + '"</span>: ';
                renderJSON(data[key], line);
                
                if (index < keys.length - 1) {
                    const comma = document.createElement('span');
                    comma.className = 'json-comma';
                    comma.textContent = ',';
                    line.appendChild(comma);
                }
                
                content.appendChild(line);
            });
            
            const closeBracket = document.createElement('div');
            closeBracket.className = 'json-bracket';
            closeBracket.textContent = '}';
            
            container.appendChild(toggle);
            container.appendChild(bracket);
            container.appendChild(content);
            container.appendChild(closeBracket);
        }
    }
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}
