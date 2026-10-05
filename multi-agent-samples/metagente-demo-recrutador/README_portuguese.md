# Triagem de Currículos: três agentes, A2A e MCP

[**ENGLISH**](README.md)

Você entrega uma pasta de currículos e o texto de uma vaga. O sistema devolve os **três melhores currículos para a vaga**, cada um com o nome do candidato e o nome do arquivo. Três agentes Metagente fazem o trabalho juntos:

```text
                                  +--A2A--> Leitor ----MCP "files"----> lista a pasta
                                  |            |  \--MCP "convert"---> lê PDF, DOCX, HTML, MD, TXT, JSON
você --run--> Recrutador ---------+            |
 (pasta + vaga)   ^                |            v resumo de cada currículo
                  |                |
                  |                +--A2A--> Avaliador --MCP "fetch"--> Wikipedia (tecnologias da vaga)
                  |                              |
                  +--------- 3 melhores ---------+
você <-- lista final --- Recrutador
```

- O **Recrutador** (`recrutador.ag`) é o coordenador. Recebe a pasta e a vaga, chama os dois outros agentes por A2A e responde com a lista. Ele só orquestra: não conversa com o modelo e não lê arquivo nenhum.
- O **Leitor** (`leitor.ag`) é um servidor que fica no ar. Usa MCP para **acessar arquivos**: lista a pasta, converte cada currículo (qualquer formato) em texto e pede ao Claude um resumo curto de cada um.
- O **Avaliador** (`avaliador.ag`) também fica no ar. Usa MCP para **pesquisar na internet**: lê na Wikipedia o que são as tecnologias da vaga, compara cada candidato com a vaga e escolhe os três melhores.
- O modelo (Claude Sonnet 5.5) e a origem da chave ficam só no `metagente.toml`. Nenhum `.ag` cita provedor ou modelo.

Cada agente declara só o que precisa: o Recrutador não tem ferramenta alguma além dos dois agentes remotos, o Leitor declara só ferramentas de arquivo e o Avaliador declara só a de busca na web.

## Estado dos testes (leia antes de rodar)

O que foi e o que não foi verificado:

| Parte | Verificado? | Como |
|-------|-------------|------|
| Servidor MCP de arquivos (`server-filesystem`) | Sim | Chamado de verdade: `list_directory`, `list_allowed_directories`, e recusa de caminho fora da pasta |
| Servidor MCP de conversão (`markitdown-mcp`) | Sim | Os 9 arquivos da pasta `curriculos/` foram convertidos em texto, incluindo os 2 PDFs e os 2 DOCX |
| Servidor MCP `fetch` | Parcial | Sobe e lista a ferramenta `fetch`; a busca na Wikipedia **não pôde ser testada** neste ambiente por restrição de rede |
| Sintaxe dos `.ag` | Só por leitura | Conferida contra `syntax.md`, `guide.md` e o exemplo City Briefing. **`metagente check` não foi executado** (não havia o binário aqui) |
| Execução dos agentes com Claude, A2A entre eles, e o ranking final | **Não** | Nada disso foi executado. A saída mostrada abaixo é o resultado **esperado**, não uma gravação |

Por isso, a seção "Rodando em etapas" propõe uma escada de testes. Comece por `metagente check` e suba degrau por degrau.

Um ponto específico a observar: o Recrutador passa textos de várias linhas (a vaga e os resumos) como valores de uma chamada A2A. O exemplo City Briefing só passa um valor curto (`city`). Se essa chamada falhar, o degrau 4 abaixo mostra onde.

## Pré-requisitos

1. **Metagente.** Na pasta do projeto (a que tem o `Cargo.toml`): `cargo build --release`. O programa é `target/release/metagente`. Os comandos abaixo dizem `metagente`; ponha essa pasta no PATH ou digite o caminho completo.
2. **Node.js**, porque o servidor MCP de arquivos é iniciado com `npx` (`npx -y @modelcontextprotocol/server-filesystem .`). Confira com `npx --version`.
3. **uv**, que inicia os servidores MCP de conversão e de `fetch` (`uvx markitdown-mcp` e `uvx mcp-server-fetch`). Confira com `uvx --version`. A primeira execução baixa os pacotes.
4. **Uma chave da API da Anthropic.** O uso é cobrado na sua conta. Uma triagem de 9 currículos faz várias chamadas ao modelo (o Leitor usa uma por passo).
5. **Três terminais**, todos abertos nesta pasta (`triagem-curriculos/`). O Metagente procura o `metagente.toml` na pasta onde é iniciado, e o servidor de arquivos também enxerga a pasta atual.

Os caminhos `file://` usados pelo conversor valem para Linux e macOS. No Windows, o Leitor precisaria de ajustes.

## Pastas e arquivos

```text
triagem-curriculos/
  README.md
  metagente.toml          configuração (modelo, chave, tempo limite)
  recrutador.ag           coordenador
  leitor.ag               agente 1: arquivos via MCP
  avaliador.ag            agente 2: internet via MCP
  vaga.txt                texto da vaga (fictícia) para usar na pergunta
  curriculos/             9 arquivos de teste, todos fictícios
    fulano-quintanilha-brasil.pdf
    beltrano-albuquerque-sousa.docx
    cicrano-valadares.md
    fulana-mendes-tavares.txt
    beltrana-cavalcante-rosa.html
    cicrana-ferraz-lobo.pdf
    ze-fulanildo-barroso.json
    beltrano-fulanildo-neto.docx
    aviso-do-rh.txt        (não é currículo, de propósito)
```

Os nomes, empresas, escolas, e-mails e telefones são inventados (`example.com` e telefones com zeros). Qualquer semelhança é coincidência.

## Configuração

Tudo que você pode querer mudar está no `metagente.toml`:

```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-5-5"          # mude só esta linha para usar outro modelo
api_key_env = "ANTHROPIC_API_KEY"    # o NOME da variável que guarda a chave

[runtime]
timeout_seconds = 180                # ler 9 currículos exige vários passos com as ferramentas
think_max_steps = 30                 # o Leitor usa 1 passo para listar e 1 por arquivo

[serve]
bind = "127.0.0.1"                   # somente este computador
```

O arquivo nunca guarda a chave. Exporte-a nos dois terminais dos servidores, porque o Leitor e o Avaliador falam com o Claude:

```bash
export ANTHROPIC_API_KEY=sua-chave-aqui
```

O `think_max_steps` está em 30 (o padrão é 10) porque o Leitor precisa de um passo por currículo. Para pastas maiores, aumente esse número e o `timeout_seconds`.

Duas coisas ficam dentro dos arquivos `.ag`, porque é lá que o Metagente as declara:

- Quais servidores MCP iniciar: `tool files from mcp "..."` e `tool convert from mcp "..."` no `leitor.ag`, e `tool fetch from mcp "..."` no `avaliador.ag`.
- Onde estão os agentes remotos: as duas linhas `remote ... at "http://127.0.0.1:808x"` no `recrutador.ag`. Elas precisam bater com as portas usadas no `serve`.

## A vaga e os currículos de teste

`vaga.txt` pede um(a) engenheiro(a) backend sênior em **Rust**: 4 anos ou mais de backend, **2 anos ou mais de Rust em produção**, REST ou gRPC, PostgreSQL, mensageria (Kafka ou RabbitMQ), Docker e Kubernetes. Desejáveis: Tokio e Axum, OpenTelemetry e Prometheus, AWS, inglês.

Os currículos foram feitos para que a resposta certa seja defensável:

| Arquivo | Formato | Perfil | Esperado |
|---------|---------|--------|----------|
| `fulano-quintanilha-brasil.pdf` | PDF, coluna única | 9 anos de backend, 4 de Rust em produção (Tokio, Axum), Kafka, PostgreSQL, Kubernetes, OpenTelemetry, inglês fluente | **1º lugar** |
| `cicrano-valadares.md` | Markdown | 5 anos de backend, 3 de Rust (Actix), RabbitMQ, PostgreSQL, Docker; Kubernetes só em homologação | **Top 3** |
| `beltrano-albuquerque-sousa.docx` | Word | 7 anos de backend, majoritariamente Go, 1 ano de Rust em produção, gRPC, Kafka, Kubernetes | **Top 3** |
| `fulana-mendes-tavares.txt` | Texto puro | 12 anos de Java e Spring, Kafka, Kubernetes; Rust só como estudo | Fora |
| `beltrano-fulanildo-neto.docx` | Word, em tabela | SRE com 8 anos, Kubernetes forte, sem Rust e sem APIs de negócio | Fora |
| `ze-fulanildo-barroso.json` | JSON (padrão JSON Resume) | Júnior com bootcamp de Rust e uma lista enorme de palavras-chave, mas zero anos em produção | Fora (teste de "palavras-chave soltas") |
| `beltrana-cavalcante-rosa.html` | HTML | Front-end com 6 anos, backend básico | Fora |
| `cicrana-ferraz-lobo.pdf` | PDF, duas colunas | Cientista de dados, 5 anos de Python | Fora |
| `aviso-do-rh.txt` | Texto | Não é currículo | Ignorado |

O primeiro lugar é firme. As posições 2 e 3 (Cicrano e Beltrano Albuquerque) são um empate técnico honesto: um tem mais Rust, o outro tem mais Kubernetes e Kafka. O conjunto dos três deve ser esse, mas a ordem entre eles pode variar de uma execução para outra.

## Rodando

**Terminal 1**: inicie o Leitor e deixe rodando.

```bash
export ANTHROPIC_API_KEY=sua-chave-aqui
metagente serve leitor.ag --a2a 8081
```

**Terminal 2**: inicie o Avaliador e deixe rodando.

```bash
export ANTHROPIC_API_KEY=sua-chave-aqui
metagente serve avaliador.ag --a2a 8082
```

**Terminal 3**: faça a pergunta ao Recrutador.

```bash
metagente run recrutador.ag find folder=resumes job="$(cat job-description.txt)"
```

O `$(cat job-description.txt)` põe o texto inteiro da vaga em um único argumento. Pode levar alguns minutos, quase todos com o Leitor abrindo os 9 arquivos. Ao terminar, pare os dois servidores com Ctrl-C.

### Resposta esperada (ilustrativa, não gravada)

```text
Os 3 melhores currículos para a vaga:
1. Fulano Quintanilha Brasil | fulano-quintanilha-brasil.pdf
2. Cicrano Valadares | cicrano-valadares.md
3. Beltrano Albuquerque Sousa | beltrano-albuquerque-sousa.docx
```

### Rodando em etapas (recomendado na primeira vez)

Como nada aqui foi executado de ponta a ponta, suba esta escada. Cada degrau isola uma parte:

1. **Sintaxe.** `metagente check leitor.ag`, `metagente check avaliador.ag` e `metagente check recrutador.ag`. Cada erro aponta a linha e diz como corrigir.
2. **Leitor sozinho** (sem A2A): `metagente run leitor.ag summarize folder=curriculos > resumos.txt`. Abra o `resumos.txt`: deve haver 9 blocos `ARQUIVO / NOME / RESUMO`, com `aviso-do-rh.txt` marcado como "não é currículo".
3. **Avaliador sozinho** (sem A2A): `metagente run avaliador.ag rank job="$(cat vaga.txt)" candidates="$(cat resumos.txt)"`. Deve sair uma lista de 3 linhas.
4. **Tudo junto.** Suba os dois servidores e rode o Recrutador, como acima. Se o degrau 4 falhar e os degraus 2 e 3 passaram, o problema está na chamada A2A com textos longos de várias linhas. Nesse caso, teste o Leitor por A2A com o `curl` da próxima seção, que tem só um valor curto.

## Conferindo um agente de fora (A2A)

Qualquer cliente A2A fala com o Leitor, não só o Recrutador. Com o Terminal 1 rodando:

```bash
# Quem é e o que sabe fazer? (o cartão do agente)
curl http://127.0.0.1:8081/.well-known/agent-card.json

# Peça o resumo dos currículos
curl -X POST http://127.0.0.1:8081/a2a -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{"message":{"messageId":"m1","role":"ROLE_USER","parts":[{"text":"summarize folder=curriculos"}]}}}'
```

O cartão deve listar uma habilidade, `summarize`. O formato do `curl` segue o do exemplo City Briefing.

## Como funciona

`recrutador.ag`:

```text
agent Recrutador
  goal "Receive a job description and answer with the three best matching resumes in a folder"
  remote Leitor at "http://127.0.0.1:8081"
  remote Avaliador at "http://127.0.0.1:8082"
  accepts find folder job  # list the three best resumes in a folder for a job description
  on find
    if not folder
      fail "Preciso do nome da pasta com os currículos"
    if not job
      fail "Preciso do texto da vaga"
    candidates = Leitor.summarize folder: folder within 240 seconds
    ranking = Avaliador.rank job: job candidates: candidates within 180 seconds
    reply "Os 3 melhores currículos para a vaga:\n{ranking}"
```

Leia em voz alta: o Recrutador entende a mensagem `find`, que traz uma pasta e uma vaga. Pede o resumo dos currículos ao Leitor, entrega esse resumo junto com a vaga ao Avaliador e responde com o que o Avaliador devolveu. As duas linhas `Leitor.summarize ...` e `Avaliador.rank ...` são chamadas A2A. O `within` dá mais tempo do que o padrão, porque ler arquivos é lento.

`leitor.ag` (resumido):

```text
agent Leitor
  goal "Read every resume in a folder, in any format, and write a short digest of each one"
  tool files from mcp "npx -y @modelcontextprotocol/server-filesystem ."
  tool convert from mcp "uvx markitdown-mcp"
  accepts summarize folder  # read all resumes in a folder and summarize each one
  on summarize
    ...
    digest = think "Você é um leitor de currículos. Use a ferramenta files para listar a pasta {folder} ... Para CADA arquivo, use a ferramenta convert ... escreva um bloco ARQUIVO / NOME / RESUMO ..."
    reply digest
```

São dois servidores MCP no mesmo agente. O `files` sabe **listar** a pasta e descobrir o caminho absoluto. Ele só lê texto, então sozinho não abre PDF nem Word. O `convert` (MarkItDown) abre qualquer formato comum e devolve texto, mas precisa do caminho absoluto em forma de `file://...`. O `think` junta as duas coisas: o modelo lista, monta o caminho de cada arquivo, converte e resume. É por isso que o limite de passos sobe para 30.

`avaliador.ag` (resumido):

```text
agent Avaliador
  goal "Compare resume digests with a job description and pick the three best matches"
  tool fetch from mcp "uvx mcp-server-fetch"
  accepts rank job candidates  # pick the three best resumes for a job description
  on rank
    ...
    ranking = think "Você é um avaliador técnico de currículos. Primeiro, identifique na vaga até 3 tecnologias principais e leia a página de cada uma na Wikipedia ... Depois compare cada candidato com a vaga ... Responda SOMENTE com as três melhores ... 1. Nome do candidato | nome-do-arquivo.ext ..."
    reply ranking
```

A pesquisa na internet aqui é modesta de propósito: o Avaliador confirma o que são as tecnologias da vaga e quais são parecidas, para dar crédito parcial a quem usa uma equivalente (Go no lugar de Rust, RabbitMQ no lugar de Kafka). Ela existe para mostrar um segundo tipo de ferramenta MCP em ação. Se a página não carregar, o Avaliador segue sem ela, e o ranking ainda funciona.

O prompt do Avaliador pede explicitamente que requisitos obrigatórios pesem mais que desejáveis, e que experiência em produção pese mais que bootcamp ou lista de palavras-chave. É isso que deve tirar o Zé Fulanildo Barroso do pódio.

## Coisas para tentar

- **Troque a vaga.** Escreva uma vaga de Java e rode de novo: a Fulana Mendes Tavares deve subir para o topo.
- **Adicione um currículo** em outro formato (por exemplo `.pptx` ou `.xlsx`, formatos que o MarkItDown costuma suportar; não testei esses dois) e veja o Leitor incluí-lo sem nenhuma mudança nos agentes.
- **Troque o modelo.** Mude `model` no `metagente.toml`. Leitor e Avaliador seguem a mudança.
- **Peça o motivo.** No `avaliador.ag`, mude o formato pedido para `1. Nome | arquivo | motivo em uma frase` e compare.
- **Quebre de propósito.** Pare o Terminal 2 e rode o Recrutador: o erro deve dizer que não alcançou o Avaliador em `http://127.0.0.1:8082`.
- **Pasta inexistente.** `folder=nao-existe`: o Leitor deve relatar que não conseguiu listar a pasta.

## Problemas comuns

| Você vê | O que significa | O que fazer |
|---------|-----------------|-------------|
| `the language model could not answer: the variable ANTHROPIC_API_KEY is not set` | O terminal do servidor não tem a chave | `export ANTHROPIC_API_KEY=...` nos Terminais 1 e 2 |
| `I could not reach http://127.0.0.1:8081/...` (ou `8082`) | O servidor correspondente não está rodando | Suba o Terminal 1 ou 2 |
| `I could not start the tool server` com `npx` | Node.js não instalado | Instale o Node.js e confira `npx --version` |
| `I could not start the tool server` com `uvx` | `uv` não instalado | Instale o uv e confira `uvx --version` |
| `the agent used 30 steps and did not finish` | Pasta grande demais para o limite | Aumente `think_max_steps` e `timeout_seconds` |
| `did not finish within ... seconds` | Modelo ou conversão lentos | Tente de novo ou aumente `timeout_seconds` e os `within` do Recrutador |
| `I could not listen for A2A on 127.0.0.1:8081` | Outra coisa usa a porta | Pare o outro programa, ou troque a porta no `serve` **e** no `recrutador.ag` |
| A lista final tem menos de 3 linhas ou vem com texto extra | O modelo não seguiu o formato | Rode de novo; se persistir, reforce o formato no prompt do Avaliador |
| Um currículo não aparece nos resumos | O modelo pulou o arquivo | Rode o degrau 2 e confira; aumente `think_max_steps` |

Se algo mais der errado, `metagente check` em cada arquivo aponta a linha do erro e diz como corrigir.

## Limites e cuidados

- **O conversor não fica preso à pasta.** O `markitdown-mcp` lê qualquer `file://` do computador. Quem restringe o Leitor à pasta é o texto do prompt, não o servidor. O servidor de arquivos, esse sim, recusa caminhos fora da pasta do projeto (testado). Para uso real, rode em uma pasta ou conta que só tenha o que pode ser lido.
- **Currículos reais são dados pessoais.** O texto de cada arquivo é enviado à API do Claude. Com candidatos de verdade, confirme a base legal (LGPD) e a política da sua empresa antes de rodar. Esta demo usa só dados fictícios.
- **O ranking é um apoio, não uma decisão.** Um modelo pode errar ou refletir vieses do texto. Use a lista para priorizar a leitura humana, não para descartar candidatos.
- **O resultado varia.** O modelo escreve de forma diferente a cada execução. A ordem das posições 2 e 3 pode trocar.

## O que esta demo mostra do Metagente

| Recurso | Onde aparece |
|---------|--------------|
| Agente chamando agente por A2A | `Leitor.summarize` e `Avaliador.rank` no `recrutador.ag`, com `within` para ajustar o tempo |
| MCP para arquivos | `tool files from mcp` e `tool convert from mcp` no `leitor.ag` |
| MCP para internet | `tool fetch from mcp` no `avaliador.ag` |
| Vários servidores MCP em um agente | O Leitor usa dois |
| `think` com ferramentas declaradas | O Leitor e o Avaliador, limitados ao que cada um declarou |
| Coordenador sem modelo | O Recrutador só orquestra, com `if`, `fail` e `reply` |
| Configuração fora dos agentes | `metagente.toml` (modelo, chave, tempo, passos) |
| `serve` com A2A | Dois servidores em portas diferentes, só neste computador |
