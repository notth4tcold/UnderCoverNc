# UnderCoverNc

Imprime capas de CD, DVD, Blu-ray e jogos **no tamanho exato**, com linhas de corte e
dobra — uma recriação para Linux do saudoso **UnderCoverXP**, com visual retrô e uma
mascote em pixel art (a Disquinha).

## O que faz

- **Duas capas lado a lado** (esquerda/direita), cada uma com seu formato:
  imprimir esquerda, direita ou ambas, **numa folha só** quando couber.
- **Abrir** clicando na capa, **arrastar e soltar** arquivos, **colar** (Ctrl+V) imagem ou
  arquivo copiado no Dolphin. Botão direito na capa: girar 90°/180° ou limpar.
- **Capa par automática**: ao abrir `Jogo front.jpg`, carrega `Jogo back.jpg` no outro lado
  (palavras-chave configuráveis: `front;frente;front + inside;-f` ↔ `back;verso;back;-b`).
- **Orientação automática** (capa de DVD deitada fica de pé com a frente para cima) e
  **detecção automática do formato** pela proporção da imagem.
- **Linhas de corte** tracejadas que vão até a margem do papel sem passar por cima das
  capas, e **marcas de dobra** das lombadas.
- **Editor de formatos** igual ao original: tamanho, posição manual esquerda/direita,
  tipo interno, rótulo redondo com furo, lombada alternativa (imprimir DVD como Slim
  9 mm/7 mm ou caixa de 6 DVDs), papel próprio (bandeja de CD), tipo de mídia e bandeja da
  impressora, marcadores `LINE`/`RECT`, milímetros ou polegadas.
- **Formatos prontos**: CD frente/verso/frente+interno, caixa de DVD, Blu-ray, slim, 6 DVDs,
  PS1 verso, PS2/Wii, PS3, PSP, rótulo de CD, foto 10×15 e **VHS** (veja abaixo).
- **Capas grandes em várias folhas**: o que não cabe no papel (clamshell, luva de VHS) é
  dividido com 12 mm de sobreposição e uma linha de emenda; gira a capa se isso poupar folhas.
- **Modelo em branco (PNG, 300 dpi)** de qualquer formato, com dobras, margem de segurança e
  "VERSO | LOMBADA | FRENTE", para desenhar sua capa no GIMP/Krita no tamanho certo.
- **Pré-visualização**, **exportar PDF** (tamanho real) e **impressão via CUPS** sem
  nenhuma escala. Página de **calibração** (quadrado de 100 mm) com ajuste fino X/Y.
- **Modo lote**: escolhe uma pasta, pareia frentes e versos e imprime/exporta tudo.
- **Skins**: Clássico 98, Fósforo verde, Monitor âmbar, Vaporwave, Bolso (4 tons) e
  Aleatória; tons de cinza e linhas de CRT opcionais. Português e inglês.

## VHS: capas para as suas gravações

| Formato | Tamanho (mm) | Folhas A4 | Fonte |
|---|---|---|---|
| VHS caixa de locadora | 257,2 × 196,9, lombada 28,6 | 1 | duplication.com (10,125" × 7,75", lombada 1,125") |
| VHS clamshell | 294,5 × 211,1, lombada 27,8 | 2 | duplication.com (encarte 11 19/32" × 8 5/16") |
| VHS luva de papelão | 273 × 233 (planificada) | 2 | vhs.texs.org |
| VHS etiqueta da frente | 78 × 47 | 1 | vhs.texs.org |
| VHS etiqueta da lombada | 147 × 17 | 1 | vhs.texs.org |
| VHS-C luva | 171 × 126 | 1 | vhs.texs.org |

Como fazer: escolha o formato, clique com o botão direito na capa › **Salvar modelo em branco
(PNG)**, desenhe por cima no GIMP/Krita (o modelo já sai deitado: verso | lombada | frente),
exporte e abra a arte no UnderCoverNc. A luva de papelão só tem o tamanho total; as dobras
dela variam conforme o fabricante, então meça a sua antes de vincar. Etiquetas: use
"Imprimir numa folha só" para sair frente + lombada juntas.

## Instalar

```sh
./install.sh              # instala dependências (pergunta antes), compila e instala em ~/.local
./install.sh -y           # o mesmo, sem perguntar
./install.sh --uninstall  # remove
```

Funciona no **Arch** (pacman) e no **Ubuntu/Debian** (apt). No Ubuntu o Rust do apt é velho
demais, então o script instala o Rust pelo rustup em `~/.cargo` (testado no Ubuntu 24.04).

Pacotes: Arch `cd packaging && makepkg -si`; Ubuntu/Debian `cargo deb` e depois
`sudo apt install ./target/debian/undercovernc_*.deb`.

Precisa de CUPS (`lp`) para imprimir direto; sem ele dá para exportar PDF.

## Uso

```sh
undercovernc                     # abre a janela
undercovernc frente.jpg verso.jpg
```

| Atalho          | Ação                     |
|-----------------|--------------------------|
| Ctrl+O          | abrir imagem da esquerda |
| Ctrl+Shift+O    | abrir imagem da direita  |
| Ctrl+V          | colar (na capa sob o mouse) |
| Ctrl+P          | imprimir esquerda        |
| Ctrl+Shift+P    | imprimir ambas           |
| Ctrl+N          | limpar tudo              |

### Medidas saindo erradas?

Em **Arquivo › Configurar impressão**, clique em **Página de calibração** e meça o
quadrado: ele deve ter 100 × 100 mm a 20 mm da borda esquerda e de cima. Se sair
deslocado, ajuste a calibração X/Y. Se sair maior ou menor, a impressora está
"ajustando à página" — desative isso no driver.

## Arquivos

- Configuração: `~/.config/undercovernc/config.json`
- Formatos: `~/.config/undercovernc/formats.json` (botão **Padrões** no editor restaura)
- Imagens coladas e o último PDF impresso: `~/.cache/undercovernc/`

## Licença

MIT. Fonte VT323 © The VT323 Project Authors, licença SIL OFL 1.1 (`assets/VT323-OFL.txt`).
