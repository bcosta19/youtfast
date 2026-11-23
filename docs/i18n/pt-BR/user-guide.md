# Guia do usuário do YouTFast

[Voltar à visão geral](README.md) · [Instalar o YouTFast](install.md) · [Detalhes das plataformas](../../distribution.md)

[English](../../user-guide.md) · **Português (Brasil)** · [Español](../es/user-guide.md) ·
[Français](../fr/user-guide.md) · [Deutsch](../de/user-guide.md) · [Italiano](../it/user-guide.md)

*Esta é uma tradução do [documento original em inglês](../../user-guide.md); se houver diferenças entre os dois, vale a versão em inglês.*

## Requisitos

| O quê | Por quê | Arch | Debian/Ubuntu |
|---|---|---|---|
| libmpv 0.42+ | Reprodução, vinculada na compilação | `mpv>=0.42.0` | `libmpv-dev` (confira a versão) |
| yt-dlp | Os streams que o mpv reproduz e as novas tentativas de reprodução | `yt-dlp` | `yt-dlp` (mantenha-o atualizado) |
| Deno / EJS correspondente do yt-dlp | Desafios de extração do YouTube | `deno` e o pacote EJS da distribuição | Veja a [configuração do yt-dlp](https://github.com/yt-dlp/yt-dlp/wiki/EJS) |
| Rust 1.98 | Fixado em `rust-toolchain.toml` | `rustup` | `rustup` |
| python-secretstorage | Opcional: entrar com o login de um navegador baseado em Chromium | `python-secretstorage` | `python3-secretstorage` |

O mpv 0.42 ou mais recente precisa ser compilado com Lua (os pacotes comuns são):
ele resolve os links do YouTube pelo script `ytdl_hook` e espera até que os
streams do yt-dlp fiquem disponíveis. Hooks mais antigos podem abri-los cedo
demais e receber HTTP 403. Mantenha o yt-dlp e o runtime JS dele no `PATH`.
Confira a versão do **pacote libmpv** instalado, não a versão da API de cliente
nem a de um executável mpv instalado à parte.
Se a sua distribuição só oferece uma libmpv mais antiga, atualize a partir de uma
fonte confiável ou use uma versão Flatpak que contenha o mpv 0.42+.
A importação do navegador também precisa de `python3` com o módulo `yt_dlp`; uma
instalação do yt-dlp só como executável não fornece esse módulo Python.

## Execução

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"   # open a link
cargo run --locked --release -- "rust egui"                      # or search
```

| Tecla | Ação |
|---|---|
| Espaço, K | Reproduzir ou pausar |
| ←, → | Voltar ou avançar 5 s |
| J, L | Voltar ou avançar 10 s |
| Ctrl+← / → (Option no macOS) | Capítulo anterior / próximo |
| Home, End; 0–9 | Início / fim; posição em dezenas de por cento |
| ↑, ↓; M | Rolar a página na vertical fora do player; volume ±5% dentro do player; silenciar (nos Shorts, ↑/↓ trocam de vídeo) |
| C; <, >; vírgula, ponto | Legendas; velocidade; quadro anterior / próximo com o vídeo pausado |
| Shift+N, Shift+P | Próximo / anterior na fonte de reprodução |
| I; P; T; Shift+A | Miniplayer; janela picture-in-picture; modo cinema; modo apenas áudio |
| F, clique duplo | Tela cheia |
| Ctrl/Cmd+K | Paleta de comandos (telas, ações do player e da página, grupos de Configurações, pesquisas recentes e histórico recente) |
| /; ? | Focar a pesquisa; abrir a ajuda dos atalhos |
| Alt+← / →; botões voltar / avançar do mouse | Navegar para trás / para frente (teclas Alt no Linux/Windows) |
| Ctrl/Cmd+R | Atualizar a página atual |
| Esc | Fechar um menu/ajuda/contagem regressiva, sair da tela cheia ou voltar |

Campos de texto e menus mantêm suas próprias teclas. Botões em foco ficam com o
Espaço, e um controle de volume em foco mantém as setas. Fora do player, ↑/↓
rolam a página ativa na vertical e J/K movem a seleção de itens (`g`/`G` pulam
para o topo ou para o fim). Segure o vídeo para reproduzir temporariamente em 2×;
ao soltar, a velocidade anterior volta sem alterar a velocidade salva. A
velocidade escolhida com `<`/`>` ou no menu **Velocidade de reprodução** do
player é lembrada entre vídeos e reinicializações; ative **Lembrar para este
canal** nesse menu para salvar uma velocidade para o canal atual (a velocidade
do canal tem precedência sobre a global, até 500 canais, e pode ser limpa em
**Configurações → Reprodução → Velocidades de reprodução por canal**).
Transmissões ao vivo permanecem em 1× no tempo real; se uma transmissão estiver
atrasada em relação ao tempo real, os controles de velocidade e o toque longo de
2× ficam disponíveis para alcançar o ao vivo e voltam a 1× ao chegar lá, sem
sobrescrever as velocidades salvas. O menu do player também oferece repetição,
tamanho/fundo das legendas, cópia de um link com o instante atual e abertura do
vídeo no navegador.

A pesquisa usa diretamente a interface InnerTube do YouTube, com resultados
separados de vídeos, canais e playlists. Os filtros de vídeo incluem data,
duração, transmissão ao vivo e ordenação.
Voltar/avançar restaura a página, a consulta, os filtros, a seleção e a posição
de rolagem. Os cards têm links para o canal, um menu e ações rápidas de Assistir
mais tarde/fila. A fila pode ser reordenada ou enxugada; o miniplayer é uma caixa
de vídeo compacta 16:9, sem moldura, no canto inferior direito (clique para voltar
à página do vídeo, ou passe o mouse para ver os controles de reproduzir/pausar e
fechar), e a janela flutuante picture-in-picture (`P` ou o botão no player) abre
uma janela separada com controles de reproduzir/pausar, fechar e **Voltar com o
vídeo para a janela principal**. Essa ação transfere a reprodução sem pausar;
se a janela principal continuar minimizada, restaure-a manualmente. No Wayland,
o aplicativo não consegue manter a janela picture-in-picture acima das outras
nem trazer a janela principal à frente ou restaurá-la. Se o compositor aceitar
regras de janelas, use o ID `youtfast-pip` para mantê-la acima das demais.
A reprodução de playlists continua além da primeira página carregada. O histórico
do dispositivo permite filtrar e remover entradas individuais.

Em uma janela larga, Configurações lista seus grupos em um índice ao lado deles:
clique em um grupo para rolar até ele. Uma linha com um interruptor pode ser
clicada em qualquer lugar, não só no interruptor.

Configurações → Menu lateral escolhe quais botões do menu lateral aparecem e em
que ordem (interruptor para ocultar, setas para mover, Restaurar padrão para
desfazer). Conta e Configurações não podem ser ocultados. Marcados com gostei
continua aparecendo só com uma conta conectada; Playlists fica disponível tanto
desconectado (mostrando playlists do dispositivo) quanto conectado.
As teclas numéricas de 1 a 9 abrem os botões na ordem em que o menu lateral os
mostra.

Configurações → Reprodução → Miniplayer ao rolar mantém o vídeo visível no
miniplayer do canto quando você rola a página para além dele para ler os
comentários. Vem desativado por padrão.
Rolar de volta mostra de novo o player na página; clicar no miniplayer volta ao
vídeo no topo da página. Fechar o miniplayer pausa a reprodução.

Configurações → Reprodução → **Títulos da comunidade (DeArrow)** e
**Miniaturas da comunidade (DeArrow — envia o ID do vídeo)** vêm desativados por
padrão. Os títulos da comunidade consultam `sponsor.ajay.app` usando apenas um
prefixo de quatro dígitos hexadecimais do SHA-256 do ID do vídeo e mostram o
título original do YouTube ao passar o cursor sobre um título substituído. As
miniaturas da comunidade enviam o ID de cada vídeo diretamente para
`dearrow-thumb.ajay.app` e voltam à miniatura do YouTube quando indisponíveis.
Todas as respostas e miniaturas do DeArrow ficam apenas em um cache limitado em
memória e nunca são salvas em disco.

Configurações → Reprodução → Buffer de vídeo escolhe entre o cache padrão do
demuxer do mpv (**Maior — 150 MiB / 50 MiB**, o padrão, que carrega vários
minutos adiantados para conexões instáveis e avanços/recuos instantâneos) e
**Menor — 50 MiB / 20 MiB** (para manter o uso de RAM em torno de ~200 MB e gastar
menos dados ao pular vídeos cedo).
O modo apenas áudio (`Shift+A` ou o botão de fones de ouvido nos controles do
player) transmite e decodifica
apenas a faixa de áudio (`ba/b` e `vid=no`) do vídeo atual, voltando ao vídeo
normal ao abrir outro vídeo ou reiniciar o app.

Configurações → Reprodução → **Pular patrocínios (SponsorBlock)** vem desativado
por padrão e consulta o SponsorBlock (`sponsor.ajay.app`) usando apenas os quatro
primeiros dígitos hexadecimais do hash SHA-256 do ID do vídeo. Quando ativado,
escolha um comportamento para cada uma das oito categorias (**Patrocínio**,
**Autopromoção**, **Pedido de interação**, **Introdução**, **Encerramento**,
**Prévia**, **Trecho sem música**, **Digressão**): **Pular automaticamente**
(padrão para patrocínios, autopromoção e pedidos de interação), **Mostrar botão de
pular** (padrão para as demais categorias), **Apenas marcar na barra de progresso**
ou **Ignorar**. Segmentos de silenciamento em categorias definidas como **Pular
automaticamente** silenciam a reprodução automaticamente dentro do trecho (quando
ainda não estava silenciada) e reativam o som ao sair do trecho, parar, abrir
outro vídeo ou alternar o som manualmente.

A página do vídeo também mostra a descrição (os timestamps levam ao ponto
indicado), os capítulos e os comentários, carregados pela InnerTube. Comentários
e respostas também exibem anexos de imagem e GIF/WebP animados. Os anexos usam o
cache limitado de miniaturas, são decodificados em segundo plano e só ficam
animados enquanto visíveis.
As páginas de canal têm abas — vídeos,
shorts, ao vivo, playlists e sobre — e uma pesquisa nos vídeos do próprio
canal.

As páginas de canal incluem um banner e a ordenação dos vídeos. As playlists
mostram a capa, os metadados e o item em reprodução. Os Shorts usam um palco em
retrato com controles de próximo/anterior e navegação pela roda do mouse. A
interface começa em inglês. Escolha English, Português (Brasil), Español,
Français, Deutsch ou Italiano em Configurações → Aparência → Idioma; a mudança
vale na hora e é mantida entre execuções, inclusive para as notificações na área
de trabalho. Configurações antigas também começam em inglês. Títulos de vídeos,
comentários e preferências de legenda mantêm seus próprios idiomas.

A página do vídeo pode mostrar uma transcrição sincronizada e pesquisável, além
de prévias em miniatura na linha do tempo, quando o YouTube fornece os metadados
públicos necessários. Essas solicitações opcionais começam sob demanda,
funcionam sem conta e usam downloads limitados. Legendas/storyboards ausentes ou
uma restrição do lado do YouTube podem deixar o recurso indisponível; a
reprodução continua pela libmpv.

Para prévias sintéticas da interface, compile com `--features demo` e execute:

```sh
cargo run --locked --features demo -- --demo --demo-view watch --demo-size 1280x800
cargo run --locked --features demo -- --demo --demo-view home --demo-light
cargo run --locked --features demo -- --demo --demo-view settings --demo-language pt-BR
cargo run --locked --features demo -- --demo --demo-view theater --demo-maximized
```

O modo demo abre uma janela isolada, sem contas, arquivos do usuário, rede,
reprodução ou registro nos controles de mídia do desktop. Ele desenha estados de
exemplo, não toda interação com o backend. `--demo-capture target/example.png`
captura essa janela e a fecha. `--demo-maximized` abre a prévia maximizada. Na
visualização do vídeo, `--demo-aspect 4:3` (também `16:9` ou `21:9`) estica a
arte sintética para outro formato; `9:16` usa o modo de retrato. As
visualizações `watch`, `theater` e `fullscreen` mostram esses layouts. Os atalhos
de reprodução seguem os
[atalhos publicados pelo YouTube](https://support.google.com/youtube/answer/7631406),
com as adaptações para listas e Shorts descritas acima.

As teclas e os painéis de mídia do desktop (MPRIS, então o `playerctl` também
funciona) controlam a reprodução, e abrir o app uma segunda vez entrega o link
ou a pesquisa à janela que já está aberta, em vez de abrir outra.

## Transmitir para TV

O seletor de TVs suporta três caminhos. TVs com YouTube que usam DIAL/Lounge usam
rede nativa e não precisam de um programa auxiliar. Isso inclui modelos Samsung,
LG, Roku, Fire TV e Android/Google TV que expõem o YouTube por DIAL; quando uma TV
oferece DIAL e Google Cast, o seletor mantém as duas opções de conexão. Os
endereços IP locais distinguem TVs com o mesmo nome. Qualquer dispositivo
cujo app do YouTube mostre **Vincular com código de TV** (Apple TV, consoles de
videogame, uma TV em outra rede ou atrás do isolamento de rede de convidados)
pode ser vinculado com esse código. O suporte a Google Cast é opcional e usa o
[`catt`](https://github.com/skorokithakis/catt) instalado à parte no `PATH`.
Por exemplo, se você já usa o pipx, instale-o com `pipx install catt`.
Siga as instruções de instalação do catt para o seu sistema operacional, confira
`catt --version` no ambiente que inicia o YouTFast e reinicie o YouTFast se tiver
mudado o `PATH`. No Windows, o YouTFast também encontra o catt onde o pip
(`pip install --user catt`) e o pipx (`pipx install catt`) o colocam —
`%APPDATA%\Python\Python3*\Scripts` e `%USERPROFILE%\.local\bin`, pastas que não
estão no `PATH` que um aplicativo de desktop herda —, então qualquer um dos
comandos basta, e um YouTFast em execução o encontra na próxima busca. Enquanto o
catt estiver ausente, o seletor mostra isso como orientação ao lado dos
resultados, em vez de uma busca falhada; os caminhos por endereço IP e código de
TV não precisam de programa auxiliar. O YouTFast nunca instala o programa
auxiliar automaticamente.
O programa auxiliar e suas dependências Python não vêm nos arquivos de release.
O Flatpak não pode usar um programa auxiliar qualquer do sistema hospedeiro; esta
integração ainda não foi validada para o pacote Flatpak.

1. Ligue a TV ou o Chromecast e conecte-o à rede local do computador.
   O isolamento de rede de convidados e o multicast bloqueado podem impedir a
   descoberta; a busca é enviada por todas as interfaces de rede privadas, então
   VPNs e bridges de contêineres não escondem a rede da TV. No Windows, o
   firewall pode descartar as respostas das TVs a uma busca em toda a rede:
   permita o YouTFast quando o Windows perguntar, ou use as opções de IP ou de
   código de TV abaixo. Esta versão aceita endereços IP privados/link-local. O
   Google Cast usa a porta 8009; o DIAL do YouTube usa o endpoint anunciado pela
   TV.
2. Abra um vídeo do YouTube no YouTFast e clique em **Transmitir para TV**, ao
   lado de **Apenas áudio** nos controles do player, ou use o botão na barra
   superior.
   A descoberta só acontece quando você abre o seletor pela primeira vez ou
   pressiona **Buscar TVs**; não há varredura de rede em segundo plano.
   TVs com YouTube são descobertas por SSDP e por endpoints DIAL verificados;
   anúncios AirPlay e Google Cast também são verificados quanto ao suporte a DIAL
   do YouTube. Se uma TV não aparecer, digite o IP dela em **Buscar uma TV com
   YouTube pelo IP** e pressione **Buscar TV**. A busca manual pede a esse
   endereço o endpoint DIAL dele e depois tenta os caminhos comuns nas portas
   8080, 8008 e 8060.
   Nenhum app é aberto durante a descoberta ou a busca por IP.
   Para vincular com um código, abra o YouTube na TV, depois **Configurações →
   Vincular com código de TV**, digite o código de 12 dígitos em **Vincular uma
   TV com um código** e pressione **Vincular TV**. O código é usado uma vez e não
   é guardado; a TV vinculada é acessada pelos servidores do YouTube e fica na
   lista até o YouTFast ser fechado.
3. Escolha o dispositivo. O YouTFast envia o link canônico do vídeo e a posição
   local arredondada para baixo em segundos inteiros. Vídeos ao vivo pedem o
   ponto ao vivo.
   O player local continua reproduzindo até a TV informar que o vídeo pedido
   está em reprodução, esperando até um minuto por anúncios e pelo seletor de
   contas da TV. No DIAL, escolher a TV entra no app do YouTube dela quando ele
   já está aberto, ou o abre, e estabelece uma conexão Lounge em memória; uma
   conexão ociosa é renovada automaticamente. Se a TV pedir para você entrar,
   escolher uma conta ou assistir como convidado, escolha na TV; o YouTFast nunca
   lida com esse login.
   Uma confirmação atrasada nunca pausa outro vídeo
   local, inclusive uma reprodução recém-aberta do mesmo vídeo.
4. Use os controles explícitos da TV no painel: reproduzir, pausar,
   voltar/avançar dez segundos, diminuir/aumentar o volume em dez pontos
   percentuais e **Parar na TV**. Os comandos são enviados um por vez; o painel
   não sincroniza continuamente o estado da TV.
5. Fechar o painel apenas o oculta. Reabra-o pela barra superior para controlar
   o mesmo alvo. **Desconectar e manter na TV**, ou fechar o YouTFast, deixa a
   reprodução na TV e descarta a sessão de controle em memória. **Parar na TV**
   pede a um receptor Cast que pare, ou fecha o app do YouTube via DIAL; uma TV
   que não permite fechá-lo, ou uma vinculada com código, para o vídeo. Nenhuma
   das duas ações retoma automaticamente o player local.

O painel é um controle remoto à parte: os controles do player local, os atalhos
do teclado e as teclas de mídia do desktop continuam controlando o computador. O
ponto no botão de transmissão indica que há um alvo retido, não que a reprodução
ainda esteja acontecendo. Uma confirmação inicial bem-sucedida é histórica;
mudanças feitas na TV não são consultadas. Um tempo esgotado ou um erro pode
ocorrer depois que a TV começou: confira a TV antes de enviar de novo. O painel
mantém um alvo incerto para que você ainda possa pará-lo ou desconectar.

Os alvos Google Cast são Chromecast, Google TV Streamer
e TVs Google TV, Android TV, LG, Samsung e outras compatíveis com o Cast
ativado. O suporte depende do modelo exato e do firmware, não da marca.
Alto-falantes conhecidos do Google (Home, Nest Mini, Nest Audio, Chromecast
Audio) ficam fora da lista; outros dispositivos Cast só de áudio ainda podem
aparecer e não são alvos de vídeo. Uma TV que não informa o volume (por exemplo,
com o som em um receiver HDMI-ARC) deixa o volume no próprio controle remoto.
Os alvos DIAL precisam de um app do YouTube funcionando que exponha um
identificador de tela e acesso ao YouTube Lounge. O AirPlay é só uma pista de
descoberta; a mídia não é enviada por AirPlay, Miracast ou DLNA. A validação
nativa no Windows/macOS e no Flatpak está pendente. A API Lounge não é
documentada e pode mudar independentemente do YouTFast; as referências do
protocolo são os projetos ytcast e casttube.

O app e a conta do YouTube na TV cuidam da reprodução, incluindo anúncios e
restrições de acesso. O YouTFast não transfere a sessão da conta, os cookies do
navegador nem a biblioteca local. Com **Pular patrocínios (SponsorBlock)**
ativado, o YouTFast também acompanha a posição na TV do vídeo enviado e manda
saltos remotos por DIAL/Lounge ou Google Cast (`catt`). Ele pula automaticamente
segmentos de salto nas categorias configuradas como **Pular automaticamente**
(segmentos de silenciamento não são silenciados na TV). Mantenha o YouTFast aberto e conectado; desconectar ou desativar o
SponsorBlock interrompe os saltos automáticos seguintes.
Isso vale para o vídeo enviado explicitamente, não para os vídeos seguintes
escolhidos pela reprodução automática da TV. Transmissões ao vivo, reprodução
pausada e falta de telemetria de posição não são puladas. As verificações
ocorrem a cada dois segundos, aproximadamente, perto de um segmento e com até dez
segundos de intervalo quando o próximo está longe. A consulta da posição na TV e
a inicialização do programa auxiliar podem atrasar um salto; os dois últimos
segundos de um segmento não são tocados. Eventos de anúncio do Lounge suprimem
saltos de patrocínio; o `catt` não expõe um indicador separado e confiável de
anúncio, então o Cast depende do estado de reprodução do receptor e da
capacidade de avançar. Isso não bloqueia os anúncios do YouTube.
A validação com TV/Chromecast físicos e nas plataformas nativas continua pendente.

As configurações locais de legendas, a reprodução automática/fila locais e as
regras de sincronização do histórico da conta não são transferidas para a TV;
o YouTube na TV pode registrar seu próprio histórico. Os resultados da descoberta
e a TV selecionada são mantidos só em memória. A saída bruta e os erros do
programa auxiliar nunca são exibidos nem registrados em log, nem mesmo com o log
detalhado.

## Sua biblioteca, com ou sem conta

**Início** mostra o que há de mais novo nas suas inscrições — o feed da conta ou
os canais RSS do dispositivo —, agrupado por dia e paginado: role até o fim e
mais vídeos chegam. Os Shorts ficam em uma prateleira acima da lista: os da conta
são marcados pelo YouTube, e os dos canais do dispositivo vêm das abas Shorts
deles.

A **filtragem do feed** em **Início** e **Inscrições** acontece localmente
neste dispositivo: o botão persistente **Ocultar assistidos** oculta vídeos
assistidos no Início (incluindo a prateleira de Shorts) e da atividade e das
prévias de cartões em Inscrições; os menus dos cartões também oferecem **Não
tenho interesse** (oculta até 5.000 vídeos, removendo os mais antigos) e **Não
recomendar este canal** (bloqueia até 1.000 canais, removendo os mais antigos),
excluindo-os de Início, Inscrições e recomendações locais. Os resultados da
pesquisa e a página própria de um canal nunca são filtrados. Revise ou desfaça
vídeos ocultos (**Limpar**) e canais bloqueados (**Desbloquear**) em
**Configurações → Histórico**.

**Sem uma conta**, o YouTFast guarda as inscrições, o histórico de exibição (com
o ponto onde cada vídeo parou, para retomá-lo), o Assistir mais tarde e as
playlists locais neste computador, em `~/.local/share/youtfast/library.json`, e
as configurações (qualidade do vídeo, volume, velocidade de reprodução) em
`~/.config/youtfast/settings.json`. Crie, renomeie, exclua e reordene playlists
do dispositivo em **Playlists** (marcadas como **Neste dispositivo** mesmo ao
entrar em uma conta) e adicione ou remova vídeos pelo menu **Salvar** da página
do vídeo ou pelo menu dos cards de vídeo. O feed do Início vem do RSS de cada
canal inscrito. Traga uma biblioteca existente pelo
[Google Takeout](https://takeout.google.com): escolha YouTube e YouTube
Music, com JSON como formato do histórico, e importe `subscriptions.csv` e
`watch-history.json`. Nada disso mexe em uma conta Google. As importações aceitam
arquivos de até 64 MiB e 20.000 inscrições únicas, e mantêm as 1.000 entradas
únicas mais recentes do histórico. A leitura e a mesclagem rodam em segundo plano.
Se a biblioteca mudar durante a mesclagem, tente importar de novo; essas mudanças
mais recentes nunca são substituídas por uma cópia mais antiga da importação.

Em **Configurações → Histórico**, **Fazer backup da biblioteca** exporta suas
inscrições do dispositivo, o histórico de exibição (incluindo posições de
retomada) e Assistir mais tarde como um arquivo JSON versionado
(`youtfast-library.json`) via **Exportar biblioteca (JSON)**, ou mescla um
backup de volta via **Importar backup** usando as mesmas regras de mesclagem em
segundo plano (limite de 64 MiB, até 20.000 inscrições únicas, 1.000 entradas
mais recentes do histórico e 5.000 vídeos de Assistir mais tarde). Contas e
sessões nunca são exportadas. **Exportar inscrições** grava as inscrições do
dispositivo como um arquivo OPML 2.0 (`youtfast-subscriptions.opml`) com a URL
do feed RSS de cada canal no YouTube via **Exportar OPML**.

**Sincronização entre dispositivos**, em **Configurações → Histórico**, conecta
dois computadores seus diretamente pelo Tailscale, NetBird ou por uma rede local
privada para mesclar inscrições do dispositivo, histórico de exibição (incluindo
posições de retomada), Assistir mais tarde, playlists do dispositivo, vídeos
ocultos e canais bloqueados. Em um computador, selecione **Nova chave**, depois
**Copiar chave**, e cole-a em **Chave de pareamento** no outro. Os dispositivos
verificam se usam a mesma chave e criptografam a conexão antes de enviar dados
da biblioteca. Deixe **Endereço do outro dispositivo** vazio e o YouTFast
encontra seus outros computadores sozinho, de três formas: pergunta na rede
local quais dispositivos têm a mesma chave, pede aos clientes de linha de
comando `tailscale` e `netbird` a lista de pares, e os dispositivos que
sincronizaram contam uns aos outros quais outros computadores conhecem.
Qualquer uma delas basta: quando um computador encontra outro, ele passa a
conhecer todos os computadores que esse outro conhece, então um computador
novo recebe a biblioteca de todos os outros poucos segundos depois que você
cola a chave. A busca se repete a cada 10 minutos e sempre que os dispositivos
conhecidos param de responder. Na rede local, a pergunta leva só a porta de
escuta e um código que prova a chave, nunca a própria chave; libere no
firewall a entrada TCP e UDP na **Porta de escuta**. Para usar só um
computador, ou quando nenhuma busca o alcança (o Flatpak não consegue executar
os clientes), informe o endereço dele (como `100.64.0.2:38452`). Ajuste
**Porta de escuta** (`1024`–`65535`, padrão `38452`, a mesma em todos os
computadores) ou **Sincronizar a cada** se necessário. Endereços IP públicos
são recusados, e contas do YouTube, sessões, horários de alertas e
configurações permanecem em cada computador. Inicie uma sincronização
imediata, que também busca os dispositivos de novo, com **Sincronizar agora**
ou **Sincronizar dispositivos agora** na paleta de comandos. Se você testou uma
versão preliminar da sincronização, atualize os dois computadores e gere uma
nova chave.

**Sincronização do histórico**, em Configurações → Histórico, vale enquanto há
uma conta conectada:

- **Separado** (o padrão) mantém separadas as listas Conta e Este dispositivo.
- **Unificado no YouTFast** abre o Histórico em **Todos**, combinando as duas
  listas sem vídeos duplicados. **Conta** e **Este dispositivo** continuam
  disponíveis como filtros. A pesquisa filtra a lista combinada; as posições de
  retomada e as marcações de assistido locais têm prioridade. Nada é enviado ao
  YouTube.
- **Unificado e enviado ao YouTube** também informa os vídeos que de fato começam
  a ser reproduzidos aqui, após dois segundos de reprodução ativa, à conta ativa
  no início da reprodução. Shorts passados rapidamente não são informados. Isso
  envia relatórios de reprodução ao Google a partir da sua conta; o Google pode
  tratar isso como acesso automatizado. Relatórios que falham não interrompem a
  reprodução. A lista da conta é atualizada na próxima vez que o Histórico for
  aberto depois de um relatório bem-sucedido.

A lista unificada mantém os títulos de dia do YouTube. Como o histórico da conta
tem rótulos de dia, e não horários exatos de exibição, a ordem entre as duas
fontes é aproximada; rótulos de dia desconhecidos mantêm a ordem da conta. O
botão de remover só está disponível para vídeos guardados neste dispositivo e
apaga apenas essa entrada local.
O histórico da conta continua somente leitura quanto à exclusão. A configuração
não tem efeito sem uma conta conectada.

Um arquivo de biblioteca ou de configurações ilegível ganha uma cópia de
segurança antes que os valores padrão possam ser salvos. Uma falha de leitura ou
de cópia de segurança interrompe a inicialização em vez de sobrescrever o
original. Os salvamentos ao sair esperam os salvamentos em andamento e impedem
que cópias mais antigas na fila substituam o estado final.

**Com suas contas do YouTube**, cada conta tem um nome e é guardada no
chaveiro em uma entrada própria: adicione várias, troque pela página da conta
ou com `a`, e aponte um login pelo navegador para um perfil (`Profile 1`,
`Default`, …). O feed, as inscrições, o histórico, o Assistir mais tarde,
os vídeos marcados com gostei e as playlists vêm da conta. O YouTube não oferece
login para apps como este, então o YouTFast usa a sessão que um navegador já tem:

- **O login do seu navegador**: o YouTFast lê os cookies do YouTube do navegador
  pelo yt-dlp, e os lê de novo sempre que expiram.
- **Um arquivo cookies.txt**: exportado de uma janela anônima, o que mantém a
  própria sessão do navegador fora disso.

A sessão fica apenas no chaveiro do sistema (Secret Service, Keychain,
Credential Manager), nunca em um arquivo ou no log. **Usar uma conta dessa forma
pode ser tratado pelo Google como acesso automatizado**: a conta pode ter de se
verificar, ter a sessão encerrada ou ser suspensa. Uma conta secundária é a
escolha segura. Inscrever-se e o Assistir mais tarde do dispositivo agem
localmente. Avaliar um vídeo ou editar uma playlist da conta pela página do vídeo
altera a conta selecionada, com reversão se o YouTube recusar. A Sincronização do
histórico pode, além disso, informar reproduções iniciadas a essa conta quando
você escolhe explicitamente **Unificado e enviado ao YouTube**; o padrão não
envia relatórios de reprodução.

Trocar de conta limpa na hora as listas e os caches privados e fecha os menus
abertos. As gravações pendentes terminam na sessão de origem antes da troca, e
respostas atrasadas não podem atualizar a interface da nova conta. Uma sessão
expirada ou uma falha de login também descarta as respostas privadas do mesmo
lote pendente. Nenhum cache de cliente é gravado em disco; só a entrada da conta
selecionada no chaveiro pode restaurar a sessão dela.

Os logs usam categorias de diagnóstico fixas em vez de nomes de contas, IDs de
vídeos, perfis de navegador, URLs ou textos de erros externos. Essa ocultação
também se aplica às dependências e a `--verbose` / `RUST_LOG`; um log mais
detalhado não desliga a proteção de privacidade. Os downloads de miniaturas são
compartilhados entre a exibição e o cache em disco, limitados a 4 MiB e quatro
milhões de pixels cada, com no máximo três tentativas automáticas para falhas
transitórias. O HTTP 429 respeita o `Retry-After` (segundos ou data HTTP); falhas
de acesso renovam a página original de comentários uma vez por comentário, sem
anexar cookies da conta aos downloads de imagens. Imagens removidas, links
inalterados/expirados, dados não suportados e limites esgotados mostram um
controle para tentar novamente. Um item corrompido do cache pode ser substituído
independentemente das outras imagens.
O cache em disco é podado durante o uso para 2.000 arquivos e 128 MiB; respostas
`no-store` só são mantidas para a exibição atual.

Os diagnósticos de todo o aplicativo ficam salvos localmente em
`$XDG_STATE_HOME/youtfast/logs` (normalmente `~/.local/state/youtfast/logs` no
Linux). O `youtfast.log` é rotacionado para `youtfast.log.1`; cada um tem no
máximo 2 MiB. Esses arquivos cobrem o aplicativo inteiro, incluindo as categorias
de falha no download de imagens e nos decodificadores. Eles não são enviados para
lugar nenhum. No Unix, o diretório é exclusivo do dono (0700) e os arquivos de
log são 0600. O mesmo filtro de privacidade é aplicado antes do stderr e dos
arquivos, inclusive para os diagnósticos das dependências. `--verbose` acrescenta
categorias do modo de animação em cache/streaming; não há registros de log por
quadro. O log começa quando este binário passa a ser dono de uma instância em
execução; recompilar não atualiza um processo existente. Para acompanhar o log
atual no Linux (incluindo a rotação):

```sh
tail -F "${XDG_STATE_HOME:-$HOME/.local/state}/youtfast/logs/youtfast.log"
```

Em plataformas sem um diretório de estado XDG, os logs usam o subdiretório
`logs` do diretório local de dados de aplicativos da plataforma para o YouTFast.
Se abrir ou gravar o arquivo falhar, os diagnósticos continuam no stderr.

A reprodução de GIF/WebP nos comentários usa um worker decodificador e uma
textura reaproveitada por anexo visível, com no máximo dois anexos ativos. Os
quadros são reduzidos para caber em 320×200, preservando a proporção. Animações
pequenas ficam em cache até cerca de 8 MiB; as mais longas continuam
sequencialmente em vez de guardar todos os quadros no tamanho original ou de
rejeitar o clipe inteiro ao atingir um limite de quadros. As reservas controladas
de quadros, canvas e texturas dividem um orçamento de 64 MiB; a contabilidade
interna do decodificador/biblioteca/driver e o cache comum de miniaturas são
memória adicional do processo.
Um quadro que leva mais de 200 ms para ser decodificado interrompe a
decodificação automática seguinte e mantém uma prévia; esse é um limite de
trabalho observado, não a interrupção de um decodificador nativo no meio de um
quadro. Animações fora da tela são liberadas, os controles de pausa interrompem
os pedidos de quadros, e
contagens finitas de repetição da origem são respeitadas. Falhas de decodificação
depois de um quadro bem-sucedido mantêm esse quadro como prévia. Bloqueios
externos, mídia removida ou uma origem inutilizável ainda podem impedir uma
prévia ou a reprodução.

## Recomendações locais

Em **Início → Para você**, abra **Preferências de recomendações locais** e
selecione **Aplicar** para salvar e recarregar. Escolha uma duração, priorize
canais conhecidos ou descubra novos, ou dê preferência a vídeos recentes ou
com menos visualizações. Separe os interesses e termos silenciados por
vírgula, com até 300 caracteres por campo. A busca por correspondências
ignora maiúsculas e minúsculas:
interesses favorecem títulos ou nomes de canais correspondentes, e termos
silenciados ocultam títulos correspondentes. O filtro de duração exclui
vídeos de duração desconhecida.

Com a conta conectada, as recomendações do YouTube continuam sendo o padrão;
ative **Usar ranking local em vez das recomendações do YouTube** para aplicar
essas preferências ao feed. Sem histórico elegível no dispositivo, você pode
autorizar o envio de até três interesses ao YouTube em buscas sem login. Depois
que você assistir ao primeiro vídeo elegível, as recomendações locais passam a
usar seu histórico ao voltar para Início. Os termos silenciados nunca saem do
dispositivo.

## Alertas de novos vídeos

As notificações do próprio YouTube não têm API, então o YouTFast verifica suas
inscrições periodicamente (a cada 15 minutos por padrão; 5, 15, 30 ou 60 em
Configurações). O sino da barra superior abre uma lista rolável sem sair da
página atual; ali você pode verificar de novo ou limpar a lista. Abrir o sino
marca os alertas atuais como vistos, mas os mantém visíveis durante essa visita.
Vídeos encontrados por uma verificação enquanto a lista está aberta aparecem
nela e também são marcados como vistos.
A entrada Notificações do menu lateral continua abrindo a página completa. Novos
vídeos também podem aparecer como notificações na área de trabalho. Tanto as
verificações quanto as notificações na área de trabalho podem ser desativadas em
Configurações; a primeira execução só dispara o relógio, então uma instalação
nova não enche você de alertas.

## Fonte da interface

Configurações → Aparência → Fonte da interface. **Sistema** (o padrão) desenha o
YouTFast com a fonte de interface do seu próprio desktop: San Francisco no macOS,
Segoe UI no Windows e, no Linux, a fonte que o seu desktop está configurado para
usar (a fonte de interface do GNOME, a configuração do GTK ou a do KDE) ou, na
falta dela, a que o `system-ui` do fontconfig indicar.
**Inter** é a fonte que vem com o YouTFast, igual em todo computador. A mudança
vale na hora. Outras escritas (árabe, chinês, devanágari, ...) são desenhadas com
as fontes instaladas no computador em qualquer caso.

No Linux, o menu lista em seguida as fontes instaladas no computador (as que o
fontconfig diz que conseguem desenhar texto em inglês): digite no campo do menu
para filtrar a lista e escolha uma para desenhar a interface com ela. Texto em
negrito e médio usa os pesos da própria família quando ela os tem. Se uma fonte
escolhida for removida depois, o YouTFast desenha com Inter e diz qual fonte está
faltando; escolha outra no mesmo menu. Windows e macOS oferecem apenas Sistema e
Inter.

Se a fonte do sistema não puder ser encontrada ou lida, o YouTFast desenha com
Inter e o grupo Aparência avisa. **Relatar isto** abre o navegador em uma nova
issue no GitHub já preenchida com a versão do YouTFast, o seu sistema
operacional, a sessão do desktop, a fonte que o seu desktop está configurado para
usar e o que `fc-match system-ui` imprime; você lê,
acrescenta a fonte que o seu desktop usa e decide se envia. O YouTFast não envia
nada por conta própria. O tempo do player mantém os dígitos de largura fixa da
Inter em qualquer das escolhas, para que ele não fique tremendo enquanto conta.

## Temas

Configurações → Aparência. **Estilo da interface** escolhe entre doze estilos
visuais aplicados ao vivo: **Cupertino Quiet** (padrão), **Liquid Glass**,
**Cinema Minimal**, **Warm Studio**, **Adwaita (GNOME)**, **Fluent Mica**,
**Industrial (Braun)**, **Technical (Tiling)**, **Tactile Modern**, **Material
Tonal**, **Paper** e **Clássico** (a aparência e o ritmo originais). Os temas
escuro e claro integrados adaptam suas cores ao estilo escolhido, enquanto
paletas personalizadas e temas do Omarchy mantêm as próprias cores e adotam as
superfícies, elevação tonal, cantos, bordas, barra flutuante do player e
transições do estilo.

**Automático** acompanha ao vivo o tema do desktop
Omarchy no Linux, e é escuro nos outros sistemas. **Escuro** e **Claro** são as
cores próprias do YouTFast. As oito paletas compartilhadas (Catppuccin, Nord, Rosé
Pine, Tokyo Night e outras) são instaladas em `~/.config/youtfast/themes` na
primeira execução, como arquivos JSON simples que você pode editar, copiar ou
apagar; adicione as suas ali no mesmo formato (uma `base` mais cores nomeadas,
veja o [fastframe-theme](https://github.com/crmne/fastframe)). Uma build a partir
do código-fonte não ativa o hook do Omarchy (`contrib/omarchy/youtfast-theme`).
Os pacotes nativos para Linux o incluem para instalação opcional. O Flatpak não
ativa hooks do sistema hospedeiro nem lê implicitamente os temas dele. O
acompanhamento nativo do tema também funciona sem o hook onde os arquivos de tema
estiverem acessíveis.

## Sobre e atualizações

Configurações → Sobre mostra a versão instalada do YouTFast, a licença e o
changelog incluído, e consulta o GitHub Releases em busca de versões estáveis
mais novas ao iniciar e a cada 12 horas enquanto estiver aberto:

- **Notificar sobre novas versões** (ativado por padrão) envia uma notificação na
  área de trabalho quando uma nova versão é publicada e destaca Configurações no
  menu lateral, com ações rápidas na notificação para atualizar, abrir Sobre ou
  **Reiniciar agora** quando estiver pronta.
- **Atualizações automáticas (minor e patch)** (desativado por padrão) baixa,
  verifica (SHA-256 contra o `checksums.txt` da versão) e instala automaticamente
  em segundo plano as atualizações minor (`0.x.0`) e patch (`0.0.x`) em
  instalações graváveis pelo usuário (Linux nativo, Windows setup/portátil,
  macOS `.app`) e, depois, mostra o changelog do que mudou em Configurações →
  Sobre.
- **Correspondência ao método de instalação**: instalações `pacman` no Arch
  atualizam via `pkexec pacman -U` ao clicar (nunca de forma automática em
  segundo plano), instalações Windows setup/portátil preparam e aplicam a
  atualização com segurança ao reiniciar/sair para que DLLs carregadas nunca
  fiquem incompatíveis, e instalações Flatpak em sandbox notificam e oferecem a
  cópia do comando em um clique em Configurações → Sobre.
- **Versões major (`x.0.0`)** nunca são instaladas automaticamente; é preciso
  clicar no botão **Atualizar** no app para confirmar e instalar a versão.

## Ajuda

O YouTube recusa os streams de um vídeo por alguns segundos depois de
entregá-los (até o horário `available_at` deles). A libmpv 0.42+ (com Lua)
espera esse horário; com uma libmpv mais antiga, o YouTFast tenta abrir de novo
o stream recusado depois de 0, 1, 3 e 7 segundos, então um vídeo pode começar
alguns segundos depois do que começaria na 0.42. Atualizar a libmpv para 0.42+,
ou usar o Flatpak, espera o stream diretamente em vez de tentar de novo.

Se a reprodução mostrar **"Não foi possível reproduzir este vídeo. O player não
identificou a causa. Tente reabrir o vídeo."**, o stream continuou recusado
depois de cerca de 11 segundos de tentativas, ou outra coisa falhou: siga os
[passos de solução de problemas da instalação](install.md#se-algo-der-errado).

Leia o [CONTRIBUTING.md](../../../CONTRIBUTING.md) para relatar bugs e o
[SECURITY.md](../../../SECURITY.md) para problemas privados de conta/segurança.
