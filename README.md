# RigRag

[Japanese](README_ja.md) | [English](README.md)

![logo](./puuka.png)

RigRag is a local tool that batch-loads office documents scattered across a folder, such as PDF, Word, PowerPoint, and Excel files, as well as Markdown, AsciiDoc, Java source code, EPUB, and more, then lets you run conversational RAG chat with a local LLM. Instead of opening and checking each file manually, users can retrieve relevant information across documents by simply entering a question. Because meeting notes, specifications, design memos, management sheets, and source code can all be referenced through a single interface regardless of file format or storage location, RigRag helps reduce the time required for information lookup and lowers the risk of missing important details. Once loaded, indexed content is stored inside the project directory, making repeated use efficient as well.

This is a Rust CLI that reads and vectorizes local files, then performs RAG chat using embedding and generation models running on Ollama. Once a target directory is indexed, you can do things like the following:

- Instantly check constraints for a feature by searching across specifications, meeting notes, and design memos
- Search mixed document sets containing formats such as `pdf`, `docx`, `pptx`, `xlsx`, `epub`, `md`, `adoc`, and `java`
- Get conversational answers grounded in indexed documents
- Run one-shot queries such as summaries, comparisons, and gap checks directly from the CLI
- Store indexes under `.rigrag` for each project and reuse them continuously
- Better understand large documents written in other languages by interacting with them through rigrag
- Operate securely in a fully local environment without relying on internet-based systems, except when downloading models for the first time
- Let the LLM generate answers using multiple calculation tools such as addition, subtraction, multiplication, division, sum, and average
- Let the LLM generate answers using a date tool that returns today's date

## Key Features

- File scanning and chunking for the target directory
- Vector storage and index management with LanceDB
- Conversational chat using RAG search results
- Answer generation in an LLM agent style with tool-calling support
- Calculation tools: add, subtract, multiply, divide, sum, and average
- Date tool: get today's date
- Document search tool
- One-shot query execution with `-q/--query`
- Query input from standard input through pipes
- Model availability checks and Ollama download guidance
- Display of document paths referenced by the LLM while generating answers
- Multithreaded embedding with configurable thread count
- Image processing and description generation with vision-capable models

## Supported File Types

The following file extensions are supported by the current implementation:

- `md` - Markdown
- `adoc` - AsciiDoc
- `pdf` - PDF
- `xlsx` - Excel (OOXML format)
- `xls` - Excel (legacy format)
- `docx` - Microsoft Word
- `pptx` - Microsoft PowerPoint
- `epub` - EPUB e-books
- `java` - Java source code
- `txt` - Text files

## Requirements

- Rust (recommended via `rustup`)
- Ollama running locally
- Network access for fetching dependencies during the first build

## Security and Local-Only Operation Policy

- rigrag is designed to work with local Ollama and local files, and does not normally require external online systems during regular use
- Indexed document data is stored under `.rigrag/` inside the target execution directory
- Internet access is mainly required during initial setup, such as downloading crates and Ollama models
- Once the required models are available locally, the tool can be operated offline

## Installing Ollama

### 1. Windows

Method A (official installer):

1. Open `https://ollama.com/download`
2. Download the Windows installer
3. Run the installer

Method B (winget):

```powershell
winget install Ollama.Ollama
```

### 2. macOS

Method A (official installer):

1. Download the macOS installer from `https://ollama.com/download`
2. Launch Ollama after installation

Method B (Homebrew):

```bash
brew install --cask ollama
```

### 3. Linux

```bash
curl -fsSL https://ollama.com/install.sh | sh
```

### 4. Verify the installation

```bash
ollama --version
```

Start Ollama and make sure the API endpoint, by default `http://localhost:11434`, is reachable.

## Model Setup

Default configuration for this project:

- **Generation model**: `qwen3.5` - a high-accuracy model with strong Japanese support
- **Embedding model**: `nomic-embed-text-v2-moe` - a high-performance multilingual embedding model

### Downloading models

When the app starts, it interactively asks whether to download the configured models if they are not already available locally.

If you want to download them manually in advance:

```bash
ollama pull qwen3.5
ollama pull nomic-embed-text-v2-moe
```

### Using different models

You can change the generation or embedding model through command-line options:

```bash
# Specify a different generation model
cargo run -- . --model llama3

# Specify a different embedding model
cargo run -- . --embedding-model mxbai-embed-large

# Change both
cargo run -- . -m mistral -e mxbai-embed-large
```

### Notes on model selection

- **Tool calling support**: To use calculation tools, the model must support tool calling. Many models such as `qwen*` and `mistral*` do.
- **Vision support**: If you enable image processing with `--picture`, you need a vision-capable model such as `llava`.
- **Language support**: Choose a model that supports the language of the documents you want to process.

## Build

```bash
cargo build
```

## Usage

### 1. Create the initial index

Index, or vectorize, the target directory. This is required before first use.

```bash
cargo run -- <target-directory> --init
```

Example:

```bash
cargo run -- . --init
```

### 2. Interactive mode

Ask questions in a chat-style interface. Conversation history is preserved, so later answers can take previous context into account.

```bash
cargo run -- <target-directory>
```

After startup, enter your question at the prompt. The following commands are available:

- `exit` - quit
- `clear` - clear the conversation history

If the LLM supports tool calling, calculation tools and search tools are invoked automatically when needed.

### 3. One-shot queries

Use `-q/--query` to ask a single question, print the result, and exit. This is useful for batch processing.

```bash
cargo run -- <target-directory> -q "your question"
```

Example:

```bash
cargo run -- . -q "Summarize the design policy in this folder"
```

### 4. Query input via pipe

You can also pass a question through standard input. This is useful in scripts or when consuming the output of another tool.

```bash
echo "What is the main content of these documents?" | cargo run -- .
```

Or:

```bash
cat questions.txt | cargo run -- .
```

## Main Options

- `--init`: rebuild the index
- `-p, --picture`: enable image processing, which requires a vision-capable model
- `-q, --query <TEXT>`: run a one-shot query
- `-u, --url <URL>`: Ollama API URL, default `http://localhost:11434`
- `-e, --embedding-model <NAME>`: embedding model, default `nomic-embed-text-v2-moe`
- `--embedding-thread <N>`: number of embedding threads, default `4`
- `-m, --model <NAME>`: generation model, default `qwen3.5`
- `-s, --sample <N>`: number of contexts to fetch in RAG search
- `-t, --temperature <VALUE>`: generation temperature from 0.0 to 2.0; when omitted, the model default is used
- `-x, --exclude <EXT1,EXT2,...>`: exclude specified extensions from processing, comma-separated
- `-v, --verbose`: verbose output
- `--single-thread`: process using a single thread

## Data Storage

The tool creates a `.rigrag/` directory under the target directory and stores the following there:

- `config.toml` - application configuration file
- `lancedb-store/` - LanceDB vector database containing embedded documents

## Tool Features

rigrag runs in an LLM agent style and can use the following built-in tools.

### Calculation tools

The LLM can call these tools when needed to perform calculations. A tool-calling capable model is required.

- **Addition** - add two numbers
- **Subtraction** - subtract one number from another
- **Multiplication** - multiply two numbers
- **Division** - divide one number by another, with protection against division by zero
- **Sum** - calculate the total of multiple numbers
- **Average** - calculate the average of multiple numbers

### Search tool

- **Document search** - run vector search over indexed documents and retrieve relevant context

## Troubleshooting

### Cannot connect to Ollama

- Make sure Ollama is running
- Start it explicitly with `ollama serve`
- Use `--url` to specify the correct endpoint, default `http://localhost:11434`
- Check the Ollama logs for more details about connection errors

### Model capability errors

If the error message indicates missing capabilities such as the following:

- **No embedding/completion capability**: the specified model does not provide the required capability
  - Specify a different model with `--embedding-model` or `--model`
  - Example: `ollama pull nomic-embed-text-v2-moe`

- **Vision capability required**: you used `--picture`, but the model does not support vision
  - Remove `--picture`, or switch to a vision-capable model
  - Example: use a vision-capable model such as `llava`

### Tool calling does not work

To use tool features, the Ollama model must support tool calling.

- If the model does not support tool calling, calculation tools will not be invoked and only normal answers will be generated
- Many tool-calling capable models are available, including `qwen*` and `mistral*`
- You can check the startup output for `support_tools: true/false`

### Index creation is slow

- CPU parallelism may be the bottleneck
  - Adjust the embedding thread count with `--embedding-thread N`
  - Tune it based on your CPU core count; the default is `4`
  - The optimal thread count depends on the Ollama model size and system specifications
- For heavy text workloads, `--single-thread` can reduce memory pressure

### Document parsing errors

If errors occur for specific file types:

- Exclude problematic extensions with `-x/--exclude`
- Example: `rigrag . -x pdf,xlsx` skips PDF and Excel files
- Check whether the affected files are valid and not corrupted

### Out of memory

When processing a large document set:

- Disable parallel processing with `--single-thread`
- Reduce the thread count, for example with `--embedding-thread 2`
- When rebuilding the index, narrow the target set after resetting with `--init`, and exclude unnecessary files with `--exclude`
