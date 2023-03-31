window.addEventListener("beforeunload", function () {
    if (socket) {
        socket.close();
    }
});

document.addEventListener('DOMContentLoaded', () => {
    // Enable tooltips
    $(function () {
        $('[data-toggle="tooltip"]').tooltip()
    });

    let lobbyOwner = null;
    let inGameTimer = 0;
    let roundTime = 0;
    let preRoundStarted = false;
    let mainStarted = false;
    let loading = false;
    let notGuesserUuid = '';
    let playerSectionVisibleOffset = 0;
    let round = null;

    const roomOptionsButton = document.getElementById("roomOptionsSm");
    if (roomOptionsButton) {
        roomOptionsButton.addEventListener('click', () => {
            $('#optionsModal').modal();
        });
    }

    const stopGameButtonClass = document.getElementsByClassName("stopGameButton");
    for (let i = 0; i < stopGameButtonClass.length; i++) {
        stopGameButtonClass[i].addEventListener('click', function () {
            socket.send(JSON.stringify({'type': 'stop-game'}));
        })
    }

    const skipRoundButtonClass = document.getElementsByClassName("skipRoundButton");
    for (let i = 0; i < skipRoundButtonClass.length; i++) {
        skipRoundButtonClass[i].addEventListener('click', function () {
            socket.send(JSON.stringify({'type': 'skip-round'}));
        })
    }

    function postGameState() {
        document.getElementById("current-song-div").style.display = 'none';
        document.getElementById("submit-div").style.display = 'none';
        document.getElementById('play-again-container').style.display = 'block';
        const playAgain = document.getElementById('play-again-button');
        playAgain.style.display = 'block';
        playAgain.addEventListener('click', function () {
            exitGame()
        })
    }

    function inGameState() {
        document.getElementById("submit-div").style.display = 'none';
        document.getElementById("current-song-div").style.display = 'block';
    }

    function choosingSongState() {
        document.getElementById("current-song-div").style.display = 'none';
        document.getElementById("submit-div").style.display = 'block';
    }

    function refreshState(load = true) {
        const canvas = document.getElementById("canvas");
        const butDiv = document.getElementById("canvas-hint");
        const button = document.getElementById('canvas-hint-but');
        butDiv.style.display = 'block';
        canvas.style.display = 'none';
        button.addEventListener('click', function () {
            butDiv.style.display = 'none';
            canvas.style.display = 'inline';
            if (load) {
                delayCallback(1000, function () {
                    loadAudio(uuid, room, refreshState, false);
                    playAudio(inGameTimer, roundTime);
                }).then();
            }
        });
    }

    function newRound() {
        notGuesserUuid = 'abc';
        $('#scoreboardModal').modal('hide');
        disableOverlay();
        createHintLine(" ");
        document.querySelector('#time').textContent = roundTime + '';
        choosingSongState();
        updatePlayerSection();
    }

    function exitGame() {
        stopAudio();
        location.reload()
    }

    socket.addEventListener("message", (event) => {
        const data = JSON.parse(event.data);
        if (data['state'] === 'join' || data['state'] === 'joined') {
            const payload = data['payload'];
            lobbyOwner = payload['owner'];
            rebuildPlayerList(payload['payload']);
            updatePlayerSection();
            if (uuid === lobbyOwner) {
                const d = document.getElementById("roomOptionsButtonCol");
                d.style.display = "block";
                const d2 = document.getElementById("roomOptionsSmCol");
                d2.style.display = "block";
            }
        }
        if (data['state'] === 'player_data') {
            lobbyOwner = data['owner'];
            rebuildPlayerList(data['payload']);
            updatePlayerSection();
            if (uuid === lobbyOwner) {
                const d = document.getElementById("roomOptionsButtonCol");
                d.style.display = "block";
                const d2 = document.getElementById("roomOptionsSmCol");
                d2.style.display = "block";
            }
        }
        if (data['state'] === 'not-allowed') {
            createToast('System says:', 'Please wait a couple seconds between actions.', 'warning');
        }
        if (data['state'] === 'game_exit') {
            stopAudio();
            createHintLine(" ");
            window.location.replace(location.protocol + '//' + document.domain + ':' + location.port);
        }
        if (data['state'] === 'game_reload') {
            delayCallback(5000, function () {
                location.reload();
            }).then();
        }
        if (data['state'] === 'start_game') {
            // this should only happen when a player starts the game when this player is not in the lobby
            location.reload();
        }
        if (data['state'] === 'song_submission_choice') {
            if (data['return_data'] === "success") {
                $("#songModal").modal('hide');
            } else {
                disableOverlay();
                userChoiceModal(data);
            }
        }
        if (data['state'] === 'game_ended') {
            stopAudio();
            createHintLine(" ");
            postGameState();

            delayCallback(4000, function () {
                const container = document.getElementById('play-again-container');
                container.scrollIntoView({behavior: 'smooth'});

                flash('play-again-button');

            }).then();
            delayCallback(30000, exitGame).then();
        }
        if (data['state'] === 'resume_audio') {
            inGameState();
            refreshState();

            mainStarted = true;
        }
        if (data['state'] === "new_turn") {
            document.title = 'Game - New Round! - GuessTheSong.io';
            if (data['not_guesser'] != null) {
                notGuesserUuid = data['not_guesser'];
                refreshState();

                mainStarted = true;
            } else {
                notGuesserUuid = 'abc';
                loadAudio(uuid, room, refreshState, true);
            }
            disableOverlay();
            createHintLine(" ");
            inGameState();
        }
        if (data['state'] === 'scoreboard') {
            if (data['round'] > data['max_rounds']) {
                document.getElementById('roundInfo').innerText = 'Game over!';
                document.getElementById('roundInfoInner').innerText = 'Game over!';

                document.title = 'Game - Game Over! - GuessTheSong.io';
            } else {
                document.getElementById('roundInfo').innerText = 'Round ' + data['round'] + ' / ' + data['max_rounds'];
                document.getElementById('roundInfoInner').innerText = 'Round ' + data['round'] + ' / ' + data['max_rounds'];

                round = data['round'] + ' / ' + data['max_rounds'];
            }
            parseScores(data['payload']);
            $('#scoreboardModal').modal('show');
            animateScores();
        }
        if (data['state'] === 'new_round') {
            stopAudio();
            const round = data['round'];
            if (round === 0) {
                newRound();
            } else {
                delayCallback(4000, newRound).then();
            }
        }

        if (data['state'] === 'loading') {
            if (loading)
                return;
            loading = true;
            enableOverlay("Loading song...");
            $('#scoreboardModal').modal('hide');
        }

        if (data['state'] === "timer") {
            disableOverlay();
            loading = false;
            inGameTimer = data['message'];
            roundTime = data['round_time'];
            notGuesserUuid = data['not_guesser'];

            const display = document.querySelector('#time');

            if (inGameTimer === (roundTime + 3)) {
                preRoundStarted = false;
                mainStarted = false;
                $('#scoreboardModal').modal('hide');
                document.getElementById("submit-div").style.display = 'none';
                document.getElementById("current-song-div").style.display = 'block';
                display.textContent = roundTime + '';
                updatePlayerSection();
            }

            if (inGameTimer > roundTime) {
                if (!preRoundStarted) {
                    stopAudio(false);
                    createHintLine(" ");
                    $("#startGameModal").modal('show');
                    const modalDisplay = document.querySelector('#modal-timer');
                    modalDisplay.textContent = (inGameTimer - roundTime) + '';
                    preRoundStarted = true;
                }
                if (preRoundStarted) {
                    const modalDisplay = document.querySelector('#modal-timer');
                    modalDisplay.textContent = (inGameTimer - roundTime) + '';
                }
            } else {
                if (!mainStarted) {
                    $("#startGameModal").modal('hide');
                    playAudio(0, roundTime);

                    // focus chat/guess input
                    document.getElementById('user_message').focus();

                    display.textContent = inGameTimer;
                    mainStarted = true;
                }
                if (mainStarted) {
                    display.textContent = inGameTimer;
                }
                createHintLine(data['hint'], data['modified']);

                if (round === null) {
                    document.title = 'Game - ' + inGameTimer.toString() + "'s - GuessTheSong.io";
                } else {
                    document.title = 'Game - ' + inGameTimer.toString() + "'s, Round: " + round + ' - GuessTheSong.io';
                }

            }
        }

        if (data['state'] === 'emoteReaction') {
            const reaction = data['reaction'];
            const uuid = data['uuid'];

            emoteReaction(uuid, reaction);
        }
    });

    function userChoiceModal(data) {
        if (data['return_data'] !== "failure") {
            let songList = JSON.parse(data['return_data']);
            document.querySelector('.song-modal-body').innerHTML = '';
            let node = document.createElement("ul");  // Create a <li> node
            node.className = 'list-group';

            let selectionButtonText;
            for (let i = 0; i < songList.length; i++) {
                const listItem = document.createElement("li");
                listItem.className = 'list-group-item';
                node.appendChild(listItem);

                const listItemText = document.createTextNode("Name: " + songList[i]['name']);
                listItem.appendChild(listItemText);

                const pageBreak = document.createElement('br');
                listItem.appendChild(pageBreak);

                listItem.setAttribute('data-toggle', 'tooltip');
                listItem.setAttribute('data-placement', 'left');
                listItem.setAttribute('title', 'Video ID: ' + songList[i]['id']);

                const listItemSubText = document.createTextNode("Original Title: " + songList[i]['video_title']);
                listItemSubText.className = 'small';
                listItem.appendChild(listItemSubText);

                const selectionButton = document.createElement('button');
                selectionButton.id = i + '';
                selectionButton.style.float = 'right';
                selectionButton.className = "btn btn-outline-primary songSelectionButton";

                listItem.appendChild(selectionButton);

                selectionButtonText = document.createTextNode("Select");
                selectionButton.appendChild(selectionButtonText);

                $(function () {
                    $('[data-toggle="tooltip"]').tooltip()
                });
            }

            const listItem = document.createElement("li");
            listItem.className = 'list-group-item text-muted';
            const listItemTimeSmall = document.createElement('small');
            const listItemTime = document.createTextNode("Processed query in " + data['time_elapsed'] + ' seconds');
            listItemTimeSmall.appendChild(listItemTime);
            listItem.appendChild(listItemTimeSmall);
            node.appendChild(listItem);

            document.querySelector(".song-modal-body").appendChild(node);

            $("#songModal").modal();

            const buttonListener = document.querySelectorAll('.songSelectionButton');

            for (let i = 0; i < buttonListener.length; i++) {
                buttonListener[i].addEventListener('click', function () {
                    if (document.contains(document.getElementById("confirmation-area"))) {
                        document.getElementById("confirmation-area").remove();
                    }

                    const confirmationArea = document.createElement("li");
                    confirmationArea.className = 'list-group-item';
                    confirmationArea.id = 'confirmation-area';
                    node.appendChild(confirmationArea);

                    const userInfoTip = document.createElement("small");
                    userInfoTip.className = "text-muted";
                    userInfoTip.id = "userInfoTip";
                    confirmationArea.appendChild(userInfoTip);

                    const userInfoTipValue = document.createTextNode("Please click submit if the title is correct, otherwise please edit it.");
                    userInfoTip.appendChild(userInfoTipValue);

                    // BOOTSTRAP input form
                    const inputDiv = document.createElement("div");
                    inputDiv.className = 'input-group';
                    confirmationArea.appendChild(inputDiv);

                    const nestedInputDiv = document.createElement("div");
                    nestedInputDiv.className = 'input-group-prepend';
                    inputDiv.appendChild(nestedInputDiv);

                    const inputSpan = document.createElement("span");
                    inputSpan.id = "";//not sure if necessary, this strange code is in the bootstrap example?
                    inputSpan.className = "input-group-text";
                    nestedInputDiv.appendChild(inputSpan);

                    const listItemSubText = document.createTextNode("Song name:");
                    inputSpan.appendChild(listItemSubText);

                    const inputArea = document.createElement("input");
                    inputArea.type = "text";
                    inputArea.className = "form-control";
                    inputArea.value = songList[buttonListener[i].id]['name'];
                    inputArea.id = 'inputArea-modal';
                    inputDiv.appendChild(inputArea);

                    const submitButtonDiv = document.createElement("div");
                    submitButtonDiv.className = "input-group-append";
                    inputDiv.appendChild(submitButtonDiv);

                    const submitButton = document.createElement("button");
                    submitButton.className = "btn btn-outline-success";
                    submitButton.type = "button";
                    submitButton.id = "final-submission";
                    submitButtonDiv.appendChild(submitButton);

                    const submitButtonText = document.createTextNode("Submit");
                    submitButton.appendChild(submitButtonText);
                    if (document.contains(document.getElementById("final-submission"))) {
                        const submitButtonListener = document.getElementById('final-submission');
                        submitButtonListener.addEventListener('click', function () {
                            socket.send(JSON.stringify({
                                'type': 'song-submission',
                                'state': 'song_request',
                                'modified-title': document.getElementById('inputArea-modal').value,
                                'original-title': songList[buttonListener[i].id]['video_title'],
                                'id': songList[buttonListener[i].id]['id'],
                                'final': 1
                            }));

                            $("#songModal").modal('hide');
                            enableOverlay('Your song has been submitted. Please wait for everyone to submit a song');
                        });
                    }

                    confirmationArea.scrollIntoView({behavior: 'smooth'});
                    $("#inputArea-modal").highlight();

                });
            }
        }
    }

    jQuery.fn.highlight = function () {
        $(this).each(function () {
            const el = $(this);
            $("<div/>")
                .width(el.outerWidth())
                .height(el.outerHeight())
                .css({
                    "position": "absolute",
                    "left": el.offset().left,
                    "top": el.offset().top,
                    "background-color": "#28a745",
                    "opacity": ".7",
                    "z-index": "9999999"
                }).appendTo('body').fadeOut(500).queue(function () {
                $(this).remove();
            });
        });
    };

    document.querySelector('#send_message').onclick = () => {
        socket.send(JSON.stringify({
            'type': 'incoming-msg',
            'state': 'chat',
            'msg': document.querySelector('#user_message').value
        }));

        typingTimer = 0;

        document.querySelector('#user_message').value = '';
    };


    // Send song submission messages
    // initial selection of song
    document.querySelector('#submit-song-name').onclick = () => {
        socket.send(JSON.stringify({
            'type': 'song-submission',
            'state': 'song_request',
            'msg': document.querySelector('#song-message').value,
            'final': 0
        }));

        document.querySelector('#song-message').value = '';
        enableOverlay('Searching');
        createToast('System says:', 'your song has been submitted! Please be patient.', 'success')
    };

    let typingTimer = 0;
    document.getElementById('user_message').addEventListener('input', function () {
        typingTimer = 4;
    });

    function typingChecker() {
        setInterval(check, 1000);
        let typing = false;

        function check() {
            if (typingTimer === -1)
                return;
            if (typingTimer === 0 && typing) {
                socket.send(JSON.stringify({'type': 'typing-status', 'typing': false}));
                typing = false;
            }
            if (typingTimer === 4 && !typing) {
                socket.send(JSON.stringify({'type': 'typing-status', 'typing': true}));
                typing = true;
            }
            typingTimer -= 1;
        }
    }

    typingChecker();

    socket.addEventListener("message", (event) => {
        const data = JSON.parse(event.data);
        if (data['state'] === 'playerTyping') {
            const uuid = data['uuid'];
            const typing = data['typing'];
            const playerDiv = document.getElementById(uuid);
            if (!playerDiv)
                return;
            if (typing) {
                const player_div_card = playerDiv.children[0];
                if (player_div_card) {
                    const speechBubble = document.createElement("embed");
                    speechBubble.className = "speech-bubble";
                    speechBubble.src = thoughtBubble;
                    player_div_card.appendChild(speechBubble);
                }
            } else {
                const player_div = playerDiv.children[0];
                if (player_div) {
                    const player_card = player_div.children[2];
                    if (player_card)
                        player_card.remove();
                }
            }
        }
    });

    function emoteReactionTrigger(reaction) {
        socket.send(JSON.stringify({
            'type': 'emote-reaction',
            'reaction': reaction
        }));
    }

    const emotes = document.getElementsByClassName('emote');
    let animating = false;
    for (let i = 0; i < emotes.length; i++) {
        const e = emotes[i];
        e.addEventListener('click', function () {
            if (animating)
                return;
            animating = true;
            emoteReactionTrigger(i);

            flash(e.id);
            delayCallback(1000, function () {
                animating = false;
            }).then();
        })
    }

    // https://stackoverflow.com/a/18971171
    function splitString(str, length) {
        const words = str.split(" ");
        for (let j = 0; j < words.length; j++) {
            const l = words[j].length;
            if (l > length) {
                let result = [], i = 0;
                while (i < l) {
                    result.push(words[j].substr(i, length));
                    i += length;
                }
                words[j] = result.join("- ");
            }
        }
        return words;
    }

    // hacky incomplete function to prevent strings being too long
    function hideOverflow(str, length = 16) {
        const vw = Math.max(document.documentElement.clientWidth || 0, window.innerWidth || 0);
        if (vw < 920)
            length = 12;
        if (vw < 776)
            length = 10;
        if (vw < 580)
            length = 8;
        if (str.length <= length)
            return str;
        const x = str.substring(0, length - 2);
        return x + '..'
    }

    // Display all incoming messages
    socket.addEventListener("message", (event) => {
        const data = JSON.parse(event.data);
        if (data['state'] === 'chat') {

            // Display current message
            if (data.msg) {
                let reconstructedMsg = [];
                const splitMsg = data.msg.split(' ');
                for (let i = 0; i < splitMsg.length; i++) {
                    if (splitMsg[i].length >= 21) {
                        const splitWords = splitString(splitMsg[i], 18);
                        reconstructedMsg = reconstructedMsg.concat(splitWords);
                    } else {
                        reconstructedMsg.push(splitMsg[i])
                    }
                }
                const msg = reconstructedMsg.join(' ');
                const p = document.createElement('p');
                const span_username = document.createElement('span');
                const span_timestamp = document.createElement('span');
                const br = document.createElement('br');
                // Display message
                p.setAttribute("class", "msg");

                span_username.setAttribute("class", "my-username");
                for (let i = 0; i < players.listOfUsers.length; i++) {
                    const player = players.listOfUsers[i];
                    if (player.uuid === data.uuid)
                        span_username.innerText = player.emoji + ' ' + data.username;
                }
                span_username.style.color = generateColourFromUUID(data.uuid);

                // HTML to append
                p.innerHTML += span_username.outerHTML + ": " + msg + br.outerHTML + span_timestamp.outerHTML;
                document.querySelector('#display-message-section').append(p);
            }
            scrollDownChatWindow();
        }
    });

    function generateColourFromUUID(uuid) {
        return ("#" + (uuid.replace(/-/g, "").slice(0, 6)).toString())
    }

    // Scroll chat window down
    function scrollDownChatWindow() {
        const chatWindow = document.querySelector(".chat .main");
        chatWindow.scrollTop = chatWindow.scrollHeight;
    }

    // Make 'enter' key submit message for chat
    let msg = document.getElementById("user_message");
    msg.addEventListener("keyup", function (event) {
        event.preventDefault();
        if (event.key === 'Enter') {
            document.getElementById("send_message").click();
        }
    });

    // Make 'enter' key submit message for submitting a song.
    let songChoice = document.getElementById("song-message");
    songChoice.addEventListener("keyup", function (event) {
        event.preventDefault();
        if (event.key === 'Enter') {
            document.getElementById("submit-song-name").click();
        }
    });

    function updatePlayerSection(rebuild = false) {
        function maxPlayersFit() {
            if (screenWidth() <= 1) {
                return 5;
            }
            return 9;
        }

        function createArrows() {
            if (players.listOfUsers.length > maxPlayersFit()) {
                const oldLeft = document.getElementById("playerSectionLeftArrow");
                if (oldLeft)
                    oldLeft.outerHTML = "";
                const playerSpawn = document.querySelector(".player-input-spawn");
                if (!playerSpawn)
                    return;

                const left = document.createElement("div");
                left.innerHTML = "<i class=\"fas fa-chevron-left\"></i>";
                left.id = "playerSectionLeftArrow";
                left.style.width = "15px";
                left.style.marginLeft = ".7%";
                left.style.display = "inline-block";
                playerSpawn.insertBefore(left, playerSpawn.firstChild);
                left.addEventListener("click", function () {
                    if (playerSectionVisibleOffset > 0) {
                        playerSectionVisibleOffset -= 1;
                        renderPlayers();
                    }
                });

                const oldRight = document.getElementById("playerSectionRightArrow");
                if (oldRight)
                    oldRight.outerHTML = "";
                const right = document.createElement("div");
                right.innerHTML = "<i class='fas fa-chevron-right'></i>";
                right.id = "playerSectionRightArrow";
                right.style.width = "15px";
                right.style.display = "inline-block";
                right.style.height = "100%";
                playerSpawn.appendChild(right);
                right.addEventListener("click", function () {
                    if (playerSectionVisibleOffset < (players.listOfUsers.length - maxPlayersFit())) {
                        playerSectionVisibleOffset += 1;
                        renderPlayers();
                    }
                })
            } else {
                const oldLeft = document.getElementById("playerSectionLeftArrow");
                if (oldLeft)
                    oldLeft.outerHTML = "";
                const oldRight = document.getElementById("playerSectionRightArrow");
                if (oldRight)
                    oldRight.outerHTML = "";
            }
        }

        function addPlayer(player) {
            const outerShellDiv = document.createElement("div");
            outerShellDiv.className = "outerShellDiv";

            let playerStatus;

            const node = document.createElement("div");  // Create a <li> node
            node.setAttribute('data-toggle', 'popover');
            node.setAttribute('data-container', 'body');
            node.setAttribute('data-placement', 'top');
            node.setAttribute('data-content', '');

            const cardImageEncapsulation = document.createElement("div");

            if (player.uuid === notGuesserUuid) {
                node.className = 'card text-black bg-dark';
                playerStatus = document.createTextNode(hideOverflow('Not guessing'));
                cardImageEncapsulation.style.backgroundColor = "rgba(0,0,0,0)";
            } else if (player.guessed) {
                node.className = 'card text-black bg-success';
                playerStatus = document.createTextNode(hideOverflow('Guessed'));
                cardImageEncapsulation.style.backgroundColor = "rgba(0,0,0,0)";
            } else if (player.song_loaded) {
                playerStatus = document.createTextNode(hideOverflow('Loaded'));
                node.className = 'card text-black bg-primary';
                cardImageEncapsulation.style.backgroundColor = "rgba(0,0,0,0)";
            } else if (player.has_song) {
                playerStatus = document.createTextNode(hideOverflow('Song added'));
                node.className = 'card text-black bg-info';
                cardImageEncapsulation.style.backgroundColor = "rgba(0,0,0,0)";
            } else {
                playerStatus = document.createTextNode(hideOverflow('Guessing'));
                node.className = 'card text-black bg-light';
                const c = idToRgb(player.uuid);
                cardImageEncapsulation.style.backgroundColor = "rgba(" + c.r + ", " + c.g + ", " + c.b + ", 0.5)";
            }

            if (player.disconnected) {
                node.style.opacity = "0.4";
                if (playerStatus) {
                    playerStatus.textContent = "Disconnected";
                }

            } else {
                node.style.opacity = "1";
            }

            node.id = player.uuid;
            outerShellDiv.appendChild(node);

            cardImageEncapsulation.className = "card-img-caption color-div";
            node.appendChild(cardImageEncapsulation);

            const imgOverlay = document.createElement("p");

            imgOverlay.className = "card-text";
            cardImageEncapsulation.appendChild(imgOverlay);

            const imgOverlayText = document.createTextNode(player.emoji);
            imgOverlay.appendChild(imgOverlayText);

            const imgHeader = document.createElement("embed");
            imgHeader.className = "card-img-top";
            imgHeader.src = backgroundImage;
            imgHeader.style.height = "100px";
            cardImageEncapsulation.appendChild(imgHeader);

            const playerDataList = document.createElement("ul");
            playerDataList.className = 'list-group list-group-flush player-data';
            node.appendChild(playerDataList);

            const listItemName = document.createElement("li");
            listItemName.className = "list-group-item ";
            playerDataList.appendChild(listItemName);
            const listItemNameTextNode = document.createTextNode(hideOverflow(player.display_name));
            if (player.uuid === uuid) {
                listItemName.style.fontWeight = 'bold';
            }
            listItemName.style.margin = "1px";
            listItemName.style.padding = "1px";
            listItemName.appendChild(listItemNameTextNode);

            const listItemPoints = document.createElement("li");
            listItemPoints.className = "list-group-item option-lg";
            listItemPoints.style.margin = "1px";
            listItemPoints.style.padding = "1px";
            playerDataList.appendChild(listItemPoints);
            const listItemPointsTextNode = document.createTextNode("Points: " + player.points);
            listItemPoints.appendChild(listItemPointsTextNode);

            const listItemGuessing = document.createElement("li");
            listItemGuessing.className = "list-group-item option-lg";
            listItemGuessing.style.margin = "1px";
            listItemGuessing.style.padding = "1px";
            playerDataList.appendChild(listItemGuessing);

            if (player.uuid === notGuesserUuid) {
                const notGuessingImg = document.getElementsByClassName('player')[0];
                while (notGuessingImg.firstChild) {
                    notGuessingImg.removeChild(notGuessingImg.lastChild);
                }
                notGuessingImg.appendChild(imgOverlayText.cloneNode());
                listItemGuessing.appendChild(playerStatus);
            } else {
                listItemGuessing.appendChild(playerStatus);
            }

            const spawn = document.querySelector(".player-input-spawn");
            if (uuid === player.uuid) {
                if (spawn)
                    spawn.insertBefore(outerShellDiv, spawn.firstChild);
            } else {
                if (spawn)
                    spawn.appendChild(outerShellDiv);
            }
            createArrows();
        }

        function renderPlayers() {
            function render() {
                for (let i = 0; i < players.listOfUsers.length; i++) {
                    const player = players.listOfUsers[i];
                    const playerDiv = document.getElementById(player.uuid);
                    if (!playerDiv)
                        continue;
                    const outerShellDiv = playerDiv.parentElement;
                    if (!outerShellDiv)
                        continue;
                    // if visible
                    if (i < playerSectionVisibleOffset || i > ((maxPlayersFit() - 1) + playerSectionVisibleOffset)) {
                        outerShellDiv.style.display = "none";
                    } else {
                        outerShellDiv.style.display = "inline-block";
                    }
                }
            }

            if ((playerSectionVisibleOffset + maxPlayersFit()) > players.listOfUsers.length) {
                while ((playerSectionVisibleOffset + maxPlayersFit()) > players.listOfUsers.length) {
                    playerSectionVisibleOffset -= 1;
                }
                render();
            }

            if (players.listOfUsers.length > maxPlayersFit()) {
                render();
            }
        }

        if (rebuild) {
            document.querySelector('.player-input-spawn').innerHTML = '';
            for (let i = 0; i < players.listOfUsers.length; i++) {
                const player = players.listOfUsers[i];
                addPlayer(player);
            }
            renderPlayers();
        } else {
            // remove players that are not in the list of users
            const playerSpawnSection = document.getElementsByClassName('player-input-spawn')[0];
            for (let child = playerSpawnSection.firstChild; child !== null; child = child.nextSibling) {
                let seen = false;
                for (let i = 0; i < players.listOfUsers.length; i++) {
                    const player = players.listOfUsers[i];
                    if (child.id === "playerSectionLeftArrow" || child.id === "playerSectionRightArrow")
                        seen = true;
                    if (player.uuid === child.firstChild.id) {
                        seen = true;
                    }
                }
                if (seen === false) {
                    createArrows();
                    child.remove();
                }
            }

            // update current players
            let encounteredNotGuesser = false;
            for (let i = 0; i < players.listOfUsers.length; i++) {
                const player = players.listOfUsers[i];
                const playerDiv = document.getElementById(player.uuid);
                if (playerDiv) {
                    let playerDataList = playerDiv.getElementsByTagName('li');
                    const colorDiv = playerDiv.getElementsByClassName("color-div")[0];
                    const c = idToRgb(player.uuid);

                    const playerPoints = playerDataList[1];
                    const playerStatus = playerDataList[2];
                    if (player.uuid === notGuesserUuid) {
                        encounteredNotGuesser = true;
                        playerDiv.className = 'card text-black bg-dark';
                        playerStatus.textContent = hideOverflow('Not guessing');

                        const notGuessingImg = document.getElementsByClassName('player')[0];

                        notGuessingImg.innerHTML = playerDiv.getElementsByClassName('card-text')[0].innerHTML;
                    } else if (player.guessed) {
                        if (!array_of_already_won_uuids.includes(player.uuid)) {
                            let my_data = {color: generateColourFromUUID(player.uuid), duration: 1000};
                            visualizer_color_buffer.push(my_data);
                            array_of_already_won_uuids.push(player.uuid)
                        }
                        someoneGuessedFrameCount = 10;
                        playerDiv.className = 'card text-black bg-success';
                        playerStatus.textContent = hideOverflow('Guessed');
                        colorDiv.style.backgroundColor = "rgba(0,0,0,0)";
                    } else if (player.song_loaded) {
                        playerStatus.textContent = hideOverflow('Loaded');
                        playerDiv.className = 'card text-black bg-primary';
                        colorDiv.style.backgroundColor = "rgba(0,0,0,0)";
                    } else if (player.has_song) {
                        playerStatus.textContent = hideOverflow('Song added');
                        playerDiv.className = 'card text-black bg-info';
                        colorDiv.style.backgroundColor = "rgba(0,0,0,0)";
                    } else if (gameState === 'Choosing song') {
                        playerStatus.textContent = hideOverflow('Choosing song');
                        playerDiv.className = 'card text-black bg-light';
                        colorDiv.style.backgroundColor = "rgba(0,0,0,0)";
                    } else {
                        playerStatus.textContent = hideOverflow('Guessing');
                        playerDiv.className = 'card text-black bg-light';
                        if (colorDiv) {
                            colorDiv.style.backgroundColor = "rgba(" + c.r + ", " + c.g + ", " + c.b + ", 0.5)";
                        }
                    }

                    if (player.disconnected) {
                        playerDiv.style.opacity = "0.4";
                        if (playerStatus) {
                            playerStatus.textContent = hideOverflow('Disconnected');
                        }
                    } else {
                        playerDiv.style.opacity = "1";
                    }

                    playerPoints.textContent = "Points: " + player.points;
                } else {
                    addPlayer(player);
                }
            }

            renderPlayers();

            if (!encounteredNotGuesser) {
                const notGuessingImg = document.getElementsByClassName('player')[0];
                if (notGuessingImg) {
                    const monsterLogo = document.getElementById("monsterLogoImg");
                    if (monsterLogo) {
                        return;
                    }

                    while (notGuessingImg.firstChild) {
                        notGuessingImg.removeChild(notGuessingImg.lastChild);
                    }
                    const img = document.createElement('img');
                    img.id = "monsterLogoImg";
                    img.src = logoSrc;
                    img.width = 75;
                    img.height = 75;
                    notGuessingImg.appendChild(img);
                }
            }
        }
    }

    function emoteReaction(uuid, reaction, showFor = 4000) {
        const option = {
            'html': true,
            'content': '<p class="popover-custom-text">' + reaction + ' </p>'
        };
        const popover = $('#' + uuid);
        popover.popover(option);
        popover.popover('enable');
        popover.popover('show');
        popover.popover('disable');

        delayCallback(showFor, function () {
            popover.popover('dispose');
        }).then();
    }

    let generated = false;

    function createHintLine(hint, modified = false) {
        let hint_p = '';
        for (let i = 0; i < hint.length; i++) {
            if (hint[i].trim() === '') {
                hint_p = hint_p + '  ';
            } else {
                hint_p = hint_p + hint[i] + ' ';
            }
        }
        if (modified)
            hint_p += '   ';

        const tooltip = document.getElementById('hint-tooltip');
        const hintSpan = document.getElementById('hint-span');

        if (hint_p.length > 74) {
            hintSpan.style.fontSize = "70%";
            hintSpan.style.overflow = "hidden";
        }
        if (hint_p.length > 82) {
            hintSpan.style.fontSize = "70%";
        }
        if (hint_p.length > 93) {
            hintSpan.style.fontSize = "60%";
        }
        if (hint_p.length > 106) {
            hintSpan.style.fontSize = "50%";
        } else {
            hintSpan.style.fontSize = "100%";
        }

        if (modified && !generated) {
            tooltip.innerHTML = '<i class="fas fa-user-edit text-white" data-toggle="tooltip" data-placement="right" title="Song name was modified"></i>';

            generated = true; // set generated to true
        }

        if (modified) {
            hintSpan.innerText = hint_p;

        } else {
            hintSpan.innerText = hint_p;
            tooltip.innerHTML = '';
            generated = false; // set generated to false
            $(function () {
                $('[data-toggle="tooltip"]').tooltip('hide')
            })
        }
    }

    function setScoreboardItem(uuid, streak, playerName, score, position, maxPoints, pointIncrease) {
        function leaderAvatar() {
            const svgContainer = document.getElementById('leader-ava-svg');
            return svgContainer.cloneNode(true)
        }

        function createLeaderAvatar(color, size) {
            const leaderAva = leaderAvatar();
            leaderAva.style.backgroundColor = color;
            leaderAva.style.visibility = 'visible';
            const svg = leaderAva.getElementsByTagName('svg')[0];
            if (size === 1) {
                svg.style.width = 72;
                svg.style.height = 72;
            } else {
                svg.style.width = 48;
                svg.style.height = 48;
            }
            return leaderAva;
        }

        function createLeaderContent(uuid, streak, position, displayName, points) {
            const div = document.createElement('div');
            div.className = 'leader-content';
            const name = document.createElement('div');
            name.className = 'leader-name';
            name.innerText = position + ': ' + displayName;
            const score = document.createElement('div');
            score.className = 'leader-score';
            score.innerText = points;
            if (pointIncrease > 0) {
                const scoreP = document.createElement('pre');
                scoreP.className = 'text-success';
                scoreP.innerText = '  +' + pointIncrease + '  ';
                scoreP.id = 'streak-popover-' + uuid + '-' + streak;
                scoreP.setAttribute('data-toggle', 'popover');
                scoreP.setAttribute('data-container', 'body');
                scoreP.setAttribute('data-placement', 'right');
                scoreP.setAttribute('data-content', '');
                score.appendChild(scoreP);
            }

            div.appendChild(name);
            div.appendChild(score);
            return div;
        }

        function createLeaderBar(percentage, color) {
            const div = document.createElement('div');
            div.className = 'leader-bar';
            const bar = document.createElement('div');
            bar.className = 'bar';
            bar.style.width = percentage;
            bar.style.backgroundColor = color;
            div.appendChild(bar);
            return div;
        }

        function createNode(uuid, streak, position, name, score, barWidth) {
            const leader = document.createElement('div');
            const row = document.createElement('div');
            row.className = 'row';
            const col = document.createElement('div');
            col.className = 'col-md-auto';
            const col2 = document.createElement('div');
            col2.className = 'col';
            leader.appendChild(row);
            if (position === 1) {
                col.appendChild(createLeaderAvatar('#ffc107', 1));
                leader.appendChild(createLeaderBar(barWidth, '#ffc107'))
            } else if (position === 2) {
                col.appendChild(createLeaderAvatar('#6c757d', 0));
                leader.appendChild(createLeaderBar(barWidth, '#6c757d'))
            } else if (position === 3) {
                col.appendChild(createLeaderAvatar('saddlebrown', 0));
                leader.appendChild(createLeaderBar(barWidth, 'saddlebrown'))
            } else {
                leader.appendChild(createLeaderBar(barWidth, 'aquamarine'))
            }
            row.appendChild(col);
            row.appendChild(col2);
            col2.appendChild(createLeaderContent(uuid, streak, position, name, score));
            return leader;
        }

        function calculateBarWidth(maxPoints, points) {
            if (points === 0 || maxPoints === 0) {
                return '0%';
            }
            if (maxPoints === points) {
                return '100%'
            } else {
                return ((points / maxPoints).toFixed(2) * 100) + '%';
            }
        }

        const modalBody = document.getElementById("scoreboardLeaders");
        const node = createNode(uuid, streak, position, playerName, score, calculateBarWidth(maxPoints, score));
        modalBody.appendChild(node);
    }

    function parseScores(scores) {
        const modalBody = document.getElementById('scoreboardLeaders');
        while (modalBody.firstChild) {
            modalBody.removeChild(modalBody.lastChild);
        }
        const ol = document.createElement('ol');
        for (let i = 0; i < scores.length; i++) {
            const data = scores[i];
            setScoreboardItem(data['uuid'],
                data['streak'],
                data['display_name'],
                data['points'],
                i + 1,
                scores[0]['points'],
                data['point_diff']);

            const streak = parseInt(data['streak'], 10);
            const uuid = data['uuid'];
            if (streak > 1) {
                let streakValue = streak + 'x🔥';
                if (streak > 5) {
                    for (let j = 5; j < streak; j++) {
                        if (j >= 7)
                            break;
                        streakValue += '🔥';
                    }
                }
                const id = 'id="streak-font-' + uuid + '"';
                const option = {
                    'html': true,
                    'content': '<p class="popover-streak" ' + id + '>' + streakValue + ' </p>'
                };
                const popover = $('#streak-popover-' + uuid + '-' + streak);
                popover.popover(option);
            }

            delayCallback(2000, function () {
            }).then();

        }
        return ol;
    }

    function animateScores() {
        function move(goalWidth, element, streak) {
            if (goalWidth === '0%')
                return;
            let width = 1;
            const id = setInterval(frame, 20);

            function frame() {
                if (width >= goalWidth) {
                    clearInterval(id);
                    if (streak === null || typeof streak === 'undefined')
                        return;
                    // double check the modal actually opened
                    if (($("#scoreboardModal").data('bs.modal') || {})._isShown) {
                        const popover = $('#' + streak.id);
                        popover.popover('enable');
                        const s = parseInt(streak.id.substring(52), 10);
                        const eId = 'streak-font-' + streak.id.substring(15, 51);
                        popover.popover('show');
                        const e = document.getElementById(eId);
                        if (e === null)
                            return;
                        e.style.fontSize = (100 + (s * 10)) + '%';
                        if (s > 2) {
                            const rgb = hslToRgb(normal(Math.max(0, (60 - (s * 10))), 360, 0), 50 / 100, 50 / 100);
                            e.style.color = 'rgb(' + rgb[0] + ',' + rgb[1] + ',' + rgb[2] + ')';
                        }

                        popover.popover('disable');
                    }
                } else {
                    width += 1;
                    element.style.width = width + "%";
                }
            }
        }

        const modalBody = document.getElementById("scoreboardLeaders");
        const leaders = modalBody.getElementsByTagName('pre');
        const bars = modalBody.getElementsByClassName('bar');
        for (let i = 0; i < bars.length; i++) {
            const goal = bars[i].style.width.slice(0, -1);
            move(goal, bars[i], leaders[i])
        }
    }

    $(document).on("click", "#emoji-picker", function (e) {
        e.stopPropagation();
        $('.intercom-composer-emoji-popover').toggleClass("active");
    });

    $(document).click(function (e) {
        if ($(e.target).attr('class') !== '.intercom-composer-emoji-popover' && $(e.target).parents(".intercom-composer-emoji-popover").length === 0) {
            $(".intercom-composer-emoji-popover").removeClass("active");
        }
    });

    $(document).on("click", ".intercom-emoji-picker-emoji", function () {
        const msg = $("#user_message");
        msg.val(msg.val() + $(this).html()); //Don't ask me How or Why this works.
    });


    $('.intercom-composer-popover-input').on('input', function () {
        const query = this.value;
        if (query !== "") {
            $(".intercom-emoji-picker-emoji:not([title*='" + query + "'])").hide();
        } else {
            $(".intercom-emoji-picker-emoji").show();
        }
    });
});
