# Oryxis — Spec v0.1

> SSH client moderno com vault criptografado, terminal nativo e sincronização descentralizada entre usuários.

---

## 1. Visão Geral

Oryxis é um cliente SSH desktop com interface moderna inspirada no Termius, construído em **Rust**. O diferencial é a sincronização de pastas de conexões/credenciais entre usuários de forma **descentralizada** (P2P), sem depender de servidor central.

### Stack

| Camada | Tecnologia | Justificativa |
|---|---|---|
| **Core / Backend** | Rust | Performance, segurança de memória, ecossistema SSH maduro |
| **Terminal Emulator** | `alacritty_terminal` (parsing) + **Iced canvas** (rendering via wgpu) | Parsing robusto do alacritty + rendering GPU nativo sem webview |
| **SSH** | `russh` (Rust nativo) | Implementação SSH2 pura em Rust, async, suporta jump hosts |
| **UI** | **Iced** (wgpu backend) | Framework UI 100% Rust, GPU-accelerated, sem webview/JS/CSS. Um binário. |
| **Font rendering** | `cosmic-text` | Shaping + layout de texto com suporte a ligatures, usado pelo Iced internamente |
| **Crypto / Vault** | `age` + `chacha20poly1305` | Criptografia moderna, sem legacy baggage |
| **P2P Sync** | `iroh` (baseado em QUIC) | Sync descentralizado da equipe do n0, feito pra isso |
| **Storage local** | SQLite (`rusqlite`) + arquivos criptografados | Metadata indexável + blobs seguros |

**Resultado**: Um único binário Rust. Zero JS. Zero webview. Zero node_modules. Rendering direto na GPU via wgpu.

### Alternativas Descartadas

- **Tauri + React/Svelte**: Funcional mas adiciona camada web inteira (JS runtime, webview, IPC overhead). Para um app focado em terminal, o overhead não se justifica.
- **Wezterm core** em vez de Alacritty: mais features built-in (multiplexing, ligatures), porém mais acoplado e difícil de extrair como lib.
- **egui**: Immediate mode — bom pra tools, mas reter estado de UI complexa (drag & drop, animações, trees) é trabalhoso. Iced é retained mode, mais natural pra apps.
- **gpui (Zed)**: Performance absurda, mas é framework interno do Zed, sem docs/suporte pra projetos externos.
- **libp2p** em vez de iroh: mais genérico mas muito mais complexo de configurar. iroh é focado em sync de dados e funciona out-of-the-box.

---

## 2. Arquitetura

```
┌──────────────────────────────────────────────────────┐
│                 Iced Application (wgpu)               │
│                                                      │
│  ┌───────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │ Connection │  │  Vault   │  │   Sync Manager   │  │
│  │  Sidebar   │  │  Panel   │  │   (P2P Status)   │  │
│  └─────┬─────┘  └────┬─────┘  └────────┬─────────┘  │
│        │              │                 │             │
│  ┌─────┴──────────────┴─────────────────┴──────────┐ │
│  │        Terminal Widget (iced::widget::Canvas)    │ │
│  │     alacritty_terminal grid → wgpu rendering    │ │
│  └─────────────────────┬───────────────────────────┘ │
│                        │ (in-process, zero IPC)      │
│  ┌─────────────┐  ┌───┴────────┐  ┌──────────────┐  │
│  │  SSH Engine  │  │  Terminal   │  │    Vault     │  │
│  │   (russh)   │  │  (alacritty │  │  (age +      │  │
│  │             │  │   _terminal)│  │   sqlite)    │  │
│  └──────┬──────┘  └────────────┘  └──────┬───────┘  │
│         │                                │           │
│  ┌──────┴──────┐              ┌──────────┴────────┐  │
│  │  Connection │              │   P2P Sync Engine  │  │
│  │  Pipeline   │              │      (iroh)        │  │
│  │ (jump/proxy)│              │                    │  │
│  └─────────────┘              └───────────────────┘  │
└──────────────────────────────────────────────────────┘
```

> **Vantagem vs Tauri**: Tudo roda no mesmo processo. Sem IPC, sem serialização, sem overhead de webview. O terminal widget lê o grid do `alacritty_terminal` diretamente da memória e renderiza via wgpu.

### Fluxo de Dados — Terminal

```
Keystroke (winit) → iced event → russh channel.write() → remote server
                                                              │
remote server → russh channel.read() → alacritty_terminal (parse ANSI/VT)
                                              │
                                    grid cells → iced Canvas widget → wgpu → GPU
```

> **Sem camada intermediária**: O widget Iced acessa o `Term` do alacritty_terminal diretamente (mesmo address space), itera sobre as cells do grid, e desenha glyphs via `cosmic-text` + wgpu. Latência mínima possível.

---

## 3. Módulos Core

### 3.1 SSH Engine

**Crate**: `russh` + `russh-keys`

#### Tipos de conexão suportados

| Tipo | Descrição | Implementação |
|---|---|---|
| **Direta** | SSH simples para host:port | `russh::client::connect()` |
| **Jump Host** | SSH via 1+ bastions intermediários | Chain de `russh` sessions com `channel_open_direct_tcpip` |
| **SOCKS Proxy** | Conexão via proxy SOCKS4/5 | `tokio-socks` → feed no transport do `russh` |
| **HTTP Proxy** | SSH via CONNECT tunnel | HTTP CONNECT handshake → tunnel TCP → `russh` |
| **Port Forward** | Local/Remote/Dynamic forwarding | `channel_open_direct_tcpip` (local), `tcpip_forward` (remote), SOCKS5 server (dynamic) |
| **Agent Forward** | Forward do ssh-agent local | `russh` agent forwarding API |
| **ProxyCommand** | Comando custom como transport | Spawn processo, pipe stdin/stdout como transport |

#### Connection Pipeline

```rust
// Pseudocódigo do pipeline de conexão
async fn connect(config: &ConnectionConfig) -> Result<Session> {
    // 1. Resolver transport layer
    let transport = match &config.proxy {
        None => TcpStream::connect(config.target).await?,
        Some(Proxy::Socks5(addr)) => socks5_connect(addr, config.target).await?,
        Some(Proxy::Http(addr)) => http_connect(addr, config.target).await?,
        Some(Proxy::Command(cmd)) => proxy_command(cmd).await?,
    };

    // 2. Jump hosts (encadeados)
    let final_transport = if config.jumps.is_empty() {
        transport
    } else {
        let mut current = transport;
        for jump in &config.jumps {
            let session = ssh_handshake(current, jump).await?;
            current = session.channel_open_direct_tcpip(
                config.target.host, config.target.port,
            ).await?.into_stream();
        }
        current
    };

    // 3. SSH handshake final
    let session = ssh_handshake(final_transport, &config.auth).await?;
    Ok(session)
}
```

#### Autenticação suportada

- Password
- Public key (RSA, Ed25519, ECDSA)
- Keyboard-interactive (2FA/TOTP)
- Agent (ssh-agent / pageant)
- Certificate-based (`ssh-*-cert-v01@openssh.com`)

### 3.2 Terminal Emulator

**Parsing**: `alacritty_terminal` (VT state machine + grid)
**Rendering**: Iced `Canvas` widget + `cosmic-text` (glyph shaping) + wgpu (GPU)

#### Referências de terminais Rust open-source

| Projeto | Abordagem | O que podemos aproveitar |
|---|---|---|
| **Alacritty** | OpenGL rendering, `alacritty_terminal` como crate separada | Crate de parsing/grid — usamos diretamente |
| **WezTerm** | OpenGL, multiplexer embutido, config Lua | Referência pra tabs/splits, serial port support |
| **Rio** | **WebGPU** (wgpu), Sixel, ligatures | Mais próximo da nossa stack (wgpu). Referência direta pra rendering approach |
| **Ghostty** | Zig + platform-native UI, GPU-accelerated | Design decisions de UX, mas stack diferente |
| **Zellij** | Terminal multiplexer, plugins WASM | Modelo de plugins se quisermos extensibilidade futura |

> **Rio é a referência mais relevante** — usa wgpu pra rendering (mesma base que Iced). Podemos estudar como o Rio renderiza glyphs, lida com ligatures e Sixel sobre wgpu, e adaptar pro nosso widget Iced.

#### Capabilities

- VT100/VT220/xterm emulation completa (via `alacritty_terminal`)
- 256 colors + truecolor (24-bit)
- Unicode/emoji rendering (via `cosmic-text` shaping)
- Font ligatures (via `cosmic-text` + font feature flags)
- Scrollback buffer configurável
- Múltiplas tabs + split panes (Iced layout)
- Busca no buffer (regex)
- Copy/paste com seleção retangular
- Sixel image protocol (inline images no terminal)
- Temas customizáveis (Dracula, One Dark, Catppuccin, etc.)

#### Terminal Widget — Arquitetura interna

```rust
// O widget core do terminal — roda no mesmo processo, sem IPC
struct TerminalWidget {
    term: Arc<Mutex<Term<EventProxy>>>,  // alacritty_terminal
    font_system: cosmic_text::FontSystem, // glyph shaping
    glyph_cache: HashMap<GlyphKey, CachedGlyph>, // atlas de glyphs
}

impl iced::widget::canvas::Program for TerminalWidget {
    fn draw(&self, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let term = self.term.lock();
        let grid = term.grid();

        // Iterar cells do grid e desenhar cada glyph
        for indexed_cell in grid.display_iter() {
            let cell = &indexed_cell.cell;
            let point = indexed_cell.point;

            // 1. Resolver cor (fg/bg, bold, dim, etc.)
            let (fg, bg) = resolve_colors(cell, &self.theme);

            // 2. Desenhar background da cell
            draw_cell_bg(frame, point, bg, cell_size);

            // 3. Shaping do glyph via cosmic-text
            let glyph = self.shape_glyph(cell.c, cell.flags);

            // 4. Renderizar glyph (do cache ou rasterizar)
            draw_glyph(frame, glyph, point, fg, cell_size);
        }

        // Cursor
        draw_cursor(frame, grid.cursor.point, &self.theme);
    }
}
```

> **Performance**: O glyph cache evita re-rasterização. Cells "sujas" (changed desde último frame) são tracked pelo `alacritty_terminal` — só redesenhamos o que mudou. Em steady state (cursor piscando), quase zero trabalho de GPU.

### 3.3 Vault (Cofre Criptografado)

O vault armazena todos os segredos do usuário com criptografia local.

#### Estrutura

```
~/.oryxis/
├── vault.db              # SQLite criptografado (sqlcipher)
├── vault.key             # Master key derivada (argon2id)
├── keys/                 # Chaves SSH criptografadas com age
│   ├── <fingerprint>.age
│   └── ...
└── config.toml           # Preferências (não sensíveis)
```

#### Modelo de dados (SQLite)

```sql
-- Hosts/Conexões
CREATE TABLE connections (
    id          TEXT PRIMARY KEY,  -- UUID
    label       TEXT NOT NULL,
    hostname    TEXT NOT NULL,
    port        INTEGER DEFAULT 22,
    username    TEXT,
    auth_method TEXT,              -- 'password' | 'key' | 'agent' | 'interactive'
    key_id      TEXT REFERENCES keys(id),
    group_id    TEXT REFERENCES groups(id),
    jump_chain  TEXT,              -- JSON array de connection IDs
    proxy       TEXT,              -- JSON: {type, host, port, auth?}
    tags        TEXT,              -- JSON array
    notes       TEXT,
    color       TEXT,              -- Hex color para UI
    last_used   INTEGER,
    created_at  INTEGER,
    updated_at  INTEGER
);

-- Chaves SSH
CREATE TABLE keys (
    id           TEXT PRIMARY KEY,
    label        TEXT NOT NULL,
    fingerprint  TEXT UNIQUE NOT NULL,
    algorithm    TEXT NOT NULL,     -- 'ed25519' | 'rsa' | 'ecdsa'
    public_key   TEXT NOT NULL,     -- Conteúdo da pub key
    file_ref     TEXT NOT NULL,     -- Path relativo em keys/
    passphrase   BLOB,             -- Criptografada, nullable
    created_at   INTEGER
);

-- Grupos/Pastas
CREATE TABLE groups (
    id        TEXT PRIMARY KEY,
    label     TEXT NOT NULL,
    parent_id TEXT REFERENCES groups(id),
    color     TEXT,
    icon      TEXT,
    sort_order INTEGER
);

-- Snippets (comandos salvos)
CREATE TABLE snippets (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    command     TEXT NOT NULL,
    description TEXT,
    tags        TEXT,
    created_at  INTEGER
);

-- Credenciais genéricas (passwords)
CREATE TABLE credentials (
    id       TEXT PRIMARY KEY,
    label    TEXT NOT NULL,
    username TEXT,
    password BLOB NOT NULL,        -- Criptografado
    notes    TEXT,
    tags     TEXT
);
```

#### Criptografia

- **Master password** → `argon2id` → master key (256-bit)
- **Dados em SQLite** → SQLCipher (AES-256-CBC page-level encryption)
- **Chaves privadas SSH** → `age` encryption com master key como recipient
- **Passwords/secrets individuais** → `chacha20poly1305` com key derivada da master key
- **Lock automático** após N minutos de inatividade (configurável)
- **Biometria** (Touch ID / Windows Hello) como unlock alternativo via APIs nativas do OS

### 3.4 Key Manager

Gerenciamento completo de chaves SSH.

#### Funcionalidades

- **Gerar** chaves (Ed25519 recomendado, RSA 4096, ECDSA)
- **Importar** chaves existentes (PEM, OpenSSH format, PuTTY PPK)
- **Exportar** public keys
- **Visualizar** fingerprint, tipo, bits, comentário
- **Associar** chaves a conexões (1 key → N connections)
- **Deploy** public key em servidores (`ssh-copy-id` automatizado)
- **Rotação** assistida (gera nova, deploya, remove antiga)
- **SSH Agent** embutido (serve chaves sem exportar do vault)

---

## 4. Sincronização Descentralizada (P2P)

### Conceito

Usuários organizam conexões em **pastas compartilháveis**. Uma pasta pode ser compartilhada com outros usuários Oryxis via convite (link/código). A sync é **P2P** usando iroh — os dados vão direto entre os peers, sem servidor central.

### Tecnologia: iroh

[iroh](https://github.com/n0-computer/iroh) é uma lib Rust para sync descentralizado baseada em:
- **QUIC** (transport rápido e seguro)
- **BLAKE3** (hashing de conteúdo)
- **Ed25519** (identidade dos peers)
- Hole punching automático (NAT traversal)
- Relay nodes públicos como fallback (não veem conteúdo, só roteiam bytes criptografados)

### Modelo de Compartilhamento

```
┌─────────────┐                    ┌─────────────┐
│   User A    │◄──── iroh sync ───▶│   User B    │
│             │     (encrypted)    │             │
│ Folder:     │                    │ Folder:     │
│ "Prod Infra"│                    │ "Prod Infra"│
│  - server1  │                    │  - server1  │
│  - server2  │                    │  - server2  │
│  - bastion  │                    │  - bastion  │
└─────────────┘                    └─────────────┘
        ▲                                  ▲
        │          ┌─────────────┐         │
        └──────────│   User C    │─────────┘
                   │             │
                   │ Folder:     │
                   │ "Prod Infra"│
                   │ (read-only) │
                   └─────────────┘
```

### Permissões

| Role | Pode | Não pode |
|---|---|---|
| **Owner** | Tudo + deletar pasta + gerenciar membros | — |
| **Editor** | Adicionar/editar/remover conexões | Gerenciar membros, deletar pasta |
| **Viewer** | Ver conexões + usar para conectar | Editar, ver passwords em plaintext |

> **Segurança**: Passwords/keys dentro de pastas compartilhadas são criptografadas com uma chave de grupo derivada. Viewers recebem uma chave que permite "usar" a credencial (o app conecta por eles) mas não extraí-la em plaintext. Isso é feito via um "sealed credential" — o app decripta internamente para conectar, mas nunca expõe o valor ao frontend para viewers.

### Fluxo de Compartilhamento

```
1. User A cria pasta "Prod Infra" e adiciona conexões
2. User A clica "Compartilhar" → Oryxis gera um ticket iroh (blob hash + encryption key)
3. User A envia o ticket para User B (copiar link, QR code, etc.)
4. User B cola o ticket no Oryxis → join no documento iroh
5. Os dois peers sincronizam automaticamente (iroh cuida do NAT traversal)
6. Edições de ambos os lados são mergeadas (CRDT — iroh usa automerge internamente)
7. Se ambos estão offline, sync acontece quando reconectarem
```

### Resolução de Conflitos

iroh usa **CRDTs** (Conflict-free Replicated Data Types) internamente, então:
- Edições simultâneas em campos **diferentes** da mesma conexão → merge automático
- Edições simultâneas no **mesmo campo** → last-writer-wins com timestamp lógico
- Deleções → tombstone (marcado como deletado, removido após todos os peers confirmarem)

### Dados sincronizados vs. locais

| Dado | Sincronizado? | Notas |
|---|---|---|
| Metadata de conexão (host, port, user, label) | Sim | Core da sync |
| Passwords | Sim (criptografado) | Sealed credentials para viewers |
| Chaves SSH privadas | **Não por padrão** | Opt-in explícito, cada peer gera a sua |
| Chaves SSH públicas | Sim | Para referência |
| Snippets da pasta | Sim | Comandos úteis do grupo |
| Histórico de sessão | Não | Local only |
| Preferências de tema | Não | Local only |

---

## 5. Interface (UI/UX)

### Design Language

Inspirado no Termius: **dark-first**, clean, com sidebar de navegação e terminal como protagonista. Toda a UI é renderizada via Iced widgets (nativos, sem CSS/HTML).

### Layout Principal

```
┌──────────────────────────────────────────────────────────────┐
│  Oryxis                                      [lock] [_][O][X]│
├────────────┬─────────────────────────────────────────────────┤
│            │  [server-1] [server-2] [+]                      │
│  [Search]  │                                                 │
│            │  $ whoami                                        │
│ > Prod     │  deploy                                         │
│   server-1 │  $ uptime                                       │
│   server-2 │   14:23:01 up 42 days, ...                      │
│   bastion  │  $ _                                            │
│            │                                                 │
│ > Staging  │                                                 │
│   web-01   │                                                 │
│   db-01    │                                                 │
│            │                                                 │
│ > Shared * │                                                 │
│   prod-gw  │                                                 │
│            │                                                 │
│ ---------- │                                                 │
│  Keys      │                                                 │
│  Snippets  │                                                 │
│  Settings  │                                                 │
├────────────┴─────────────────────────────────────────────────┤
│  * Connected to server-1 (192.168.1.10) via bastion | 2.3ms │
└──────────────────────────────────────────────────────────────┘
```

### Componentes Iced

| Componente | Widget Iced | Notas |
|---|---|---|
| Sidebar (connection tree) | `iced::widget::Column` + custom tree widget | Colapsável, drag & drop via pointer events |
| Terminal | `iced::widget::Canvas` (custom) | Nosso widget principal — renderiza grid do alacritty |
| Tabs | `iced::widget::Row` + buttons | Tab bar com close/reorder |
| Split panes | Custom widget com drag handle | Horizontal/vertical split |
| Forms (connection editor) | `TextInput`, `PickList`, `Button` | Iced built-in widgets |
| Quick Connect overlay | Custom overlay widget | Cmd+K popup com fuzzy search |
| Modal dialogs | `iced_aw::Modal` ou custom overlay | Confirmações, share dialog |
| Scrollbar | `iced::widget::Scrollable` | Sidebar + scrollback do terminal |

### Telas Principais

1. **Dashboard** — Conexões recentes, favoritas, status de peers online
2. **Connection List** — Sidebar com árvore de grupos/pastas, drag & drop, search
3. **Terminal** — Tabs + split (horizontal/vertical), fullscreen mode
4. **Connection Editor** — Form para criar/editar conexão (com teste de conexão)
5. **Vault / Keys** — Lista de chaves, importar/gerar, associar a hosts
6. **Snippets** — Biblioteca de comandos com autocomplete no terminal
7. **Sync / Sharing** — Pastas compartilhadas, peers online, histórico de sync
8. **Settings** — Tema, terminal (font, cursor, scrollback), segurança, shortcuts

### UX Detalhes

- **Quick Connect** (Cmd/Ctrl+K): Overlay de busca universal — digita hostname, label, ou tag → conecta
- **Inline SFTP**: Drag & drop de arquivo na janela → upload via SCP/SFTP
- **Connection health**: Indicator de cor em tempo real na sidebar
- **Multi-exec**: Selecionar múltiplos hosts → executar comando em todos simultâneo
- **Session recording**: Gravar sessão como asciicast (replay no app)
- **Theming**: Sistema de temas via struct Rust (colors, spacing, fonts), importar/exportar como TOML

---

## 6. Segurança

### Princípios

1. **Zero knowledge local**: Master password nunca sai do dispositivo
2. **Vault locked by default**: App inicia travado, requer auth
3. **Memory safety**: Rust previne buffer overflows; secrets em `secrecy` crate (zeroize on drop)
4. **Minimal trust P2P**: Peers só veem dados das pastas compartilhadas com eles
5. **No phone home**: Nenhum telemetry, nenhum servidor central obrigatório

### Threat Model

| Ameaça | Mitigação |
|---|---|
| Roubo do dispositivo | Vault criptografado, auto-lock, argon2id slow KDF |
| MITM no SSH | Verificação de host key (TOFU + pinning), alertas visuais |
| Peer malicioso no sync | Permissões enforced localmente, signed entries |
| Malware lendo memória | `secrecy` crate com zeroize, mlock onde possível |
| Relay node comprometido | Dados já criptografados end-to-end antes do relay |

---

## 7. Estrutura do Projeto

```
oryxis/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── oryxis-core/            # Tipos compartilhados, models, errors
│   ├── oryxis-ssh/             # SSH engine (russh wrapper)
│   ├── oryxis-terminal/        # Terminal widget (alacritty_terminal + iced canvas)
│   ├── oryxis-vault/           # Vault + crypto + key management
│   ├── oryxis-sync/            # P2P sync engine (iroh wrapper)
│   └── oryxis-app/             # Iced application (views, state, main loop)
│       └── src/
│           ├── main.rs          # Entry point
│           ├── app.rs           # Iced Application impl
│           ├── views/           # Telas (dashboard, terminal, vault, settings)
│           ├── widgets/         # Widgets custom (sidebar tree, split pane, quick connect)
│           ├── theme.rs         # Sistema de temas
│           └── state.rs         # Estado global da aplicação
├── resources/                  # Ícones, fontes, default themes (TOML)
└── SPEC.md                     # Este arquivo
```

---

## 8. Dependências Principais (Rust)

```toml
[workspace.dependencies]
# UI — 100% Rust, GPU-accelerated
iced = { version = "0.13", features = ["canvas", "tokio", "advanced"] }
iced_aw = "0.11"                # Widgets extras (modal, tabs, etc.)

# SSH
russh = "0.46"
russh-keys = "0.46"

# Terminal parsing
alacritty_terminal = "0.24"

# Font shaping (usado pelo iced internamente, mas precisamos direto pro terminal)
cosmic-text = "0.12"

# Crypto
age = "0.10"
argon2 = "0.5"
chacha20poly1305 = "0.10"
secrecy = "0.8"

# P2P
iroh = "0.30"
iroh-blobs = "0.30"

# Storage
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }

# Async
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Utils
uuid = { version = "1", features = ["v4"] }
tracing = "0.1"
thiserror = "2"
```

---

## 9. MVP (v0.1) — Escopo Mínimo

### Incluso no MVP

- [ ] Vault básico (master password, SQLCipher, lock/unlock)
- [ ] CRUD de conexões (host, port, user, password/key)
- [ ] Conexão SSH direta (password + key auth)
- [ ] Terminal funcional (alacritty_terminal + iced canvas widget)
- [ ] Múltiplas tabs
- [ ] Gerenciamento de chaves (gerar Ed25519, importar, listar)
- [ ] Grupos/pastas para organizar conexões
- [ ] Quick Connect (Cmd+K)
- [ ] Tema dark padrão estilo Termius

### Pós-MVP (v0.2+)

- [ ] Jump hosts e proxy support
- [ ] SFTP / file transfer
- [ ] Split panes
- [ ] P2P sync (iroh)
- [ ] Pastas compartilhadas com permissões
- [ ] Snippets
- [ ] Multi-exec
- [ ] Session recording
- [ ] Biometric unlock
- [ ] Port forwarding UI
- [ ] Temas customizáveis
- [ ] Export/import de configurações

---

## 10. Decisões em Aberto

1. **SQLCipher vs criptografia aplicação-level**: SQLCipher é transparente mas adiciona ~2MB ao bundle e depende de C linkage. Alternativa: SQLite plain + criptografar/decriptar campos manualmente. Decisão provisória: SQLCipher pela simplicidade.

2. **Iroh versão**: Iroh está em desenvolvimento ativo e a API muda entre versões. Precisamos pinnar uma versão estável e monitorar breaking changes.

3. **Licença**: MIT? GPL? AGPL? — Definir antes de publicar código.

4. **Nome das pastas compartilhadas**: "Vaults compartilhados"? "Teams"? "Spaces"? — Definir terminologia.

5. **Terminal widget: Canvas puro vs shader custom**: Começar com `iced::widget::Canvas` (2D drawing). Se performance não for suficiente pra scrollback grande, migrar pra shader custom com glyph atlas texture (approach do Rio/Alacritty).
