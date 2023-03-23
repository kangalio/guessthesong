window.addEventListener("beforeunload", function () {
    socket.close();
});

let lobbyOwner = null;

$(function () {
    $('[data-toggle="tooltip"]').tooltip()
});

function playerJoinOrLeave(payload, owner) {
    lobbyOwner = owner;
    rebuildPlayerList(payload);
    buildPlayerList();

    const selector = $("#startGameButton");
    const button = document.getElementById("startGameButton");
    const button2 = document.getElementById("startGameSpinner");
    if (uuid !== lobbyOwner) {

        selector.addClass("disabled");
        button.style.display = "none";
        button2.style.display = "block";
    } else {
        button.style.display = "block";
        if (button2.style.display === "block") {
            location.reload();
        }

        button2.style.display = "none";
        selector.removeClass("disabled");
        selector.off('click');
        selector.on('click', function () {
            changeGameState();
        });
        selector.html("Start Game!");
    }
}

document.addEventListener('DOMContentLoaded', () => {

    socket.on('message', raw_data => {
        const data = cenOrOri(raw_data);
        if (data['state'] === 'join') {
            const payload = data['payload'];
            playerJoinOrLeave(payload['payload'], payload['owner']);
        }
        if (data['state'] === 'player_data') {
        }
        if (data['state'] === 'joined') {
            const payload = data['payload'];
            playerJoinOrLeave(payload['payload'], payload['owner']);
        }
        if (data['state'] === 'leave') {
            const payload = data['payload'];
            playerJoinOrLeave(payload['payload'], payload['owner']);
        }
        if (data['state'] === 'game_options') {
            const options = data['data'];
            let subtitle = options['mode'] + ' | ';

            if (options['theme'] !== null) {
                subtitle = subtitle + options['theme'] + " | ";
            }

            if (options['round_time'] !== null) {
                subtitle = subtitle + options['round_time'] + "s rounds | ";
            }

            subtitle = subtitle + options['rounds'] + " rounds";
            const subtitleElement = document.getElementById("roomOptions");
            if (subtitleElement !== null) {
                subtitleElement.innerText = subtitle;
            }
        }

        if (data['state'] === 'start_game') {
            location.reload();
        }

        if (data['state'] === 'game_reload') {
            reloaded();
        }
    });
});

function changeGameState() {
    socket.emit('start-game');
}

function buildPlayerList() {
    document.querySelector('.list-group').innerHTML = '';
    const node = document.createElement("li");
    node.className = 'list-group-item  bg-light';

    for (let i = 0; i < players.listOfUsers.length; i++) {
        const player = players.listOfUsers[i];
        const node = document.createElement("li");
        node.className = 'list-group-item  bg-light';
        node.id = player.uuid;
        const textNode = document.createTextNode(player.emoji + " " + player.display_name);
        node.appendChild(textNode);
        document.querySelector(".list-group").appendChild(node);
    }

    const owner = document.getElementById(lobbyOwner);
    if (owner) {
        owner.innerHTML += " <span class=\"badge \" style='font-size: small; color: #ffc107;' data-toggle=\"tooltip\" data-placement=\"top\" title=\"This user is the lobby owner\"><i class=\"far fa-star\"></i></span>";
    }
}

function myFunction() {
    const copyText = document.getElementById("roomPinPlaceholder");
    copyText.select();
    copyText.setSelectionRange(0, 99999);
    document.execCommand("copy");
}

function myFunction2() {
    const copyText = document.getElementById("roomPassPlaceholder");
    copyText.select();
    copyText.setSelectionRange(0, 99999);
    document.execCommand("copy");
}

function myFunction3() {
    const copyText = document.getElementById("roomURLPlaceholder");
    copyText.select();
    copyText.setSelectionRange(0, 99999);
    document.execCommand("copy");
}

document.addEventListener('DOMContentLoaded', () => {
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

    // Make 'enter' key submit message for chat
    let msg = document.getElementById("user_message");
    msg.addEventListener("keyup", function (event) {
        event.preventDefault();
        if (event.key === 'Enter') {
            document.getElementById("send_message").click();
        }
    });

    document.querySelector('#send_message').onclick = () => {
        socket.emit('incoming-msg', {
            'state': 'chat',
            'msg': document.querySelector('#user_message').value
        });

        document.querySelector('#user_message').value = '';
    };

    function generateColourFromUUID(uuid) {
        return ("#" + (uuid.replace(/-/g, "").slice(0, 6)).toString())
    }

// Scroll chat window down
    function scrollDownChatWindow() {
        const chatWindow = document.querySelector(".chat-main");
        chatWindow.scrollTop = chatWindow.scrollHeight;
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

// Display all incoming messages
    socket.on('message', raw_data => {
        const data = cenOrOri(raw_data);
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
});
