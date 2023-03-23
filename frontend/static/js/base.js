initNightmodeCookie();
let gameState = null;

let censored = 'cen';
if (getCookie('disable_word_censor') === '')
    censored = 'cen';
else
    censored = 'ori';

function cenOrOri(raw) {
    const a = raw[censored];
    if (censored === 'cen')
        return JSON.parse(a);
    return a
}

document.addEventListener('DOMContentLoaded', () => {
    let disconnected = false;
    if (typeof socket === 'undefined') {
        return;
    }
    socket.on('connect_error', () => {
        disconnected = true;
    });

    socket.on('reconnect', () => {
        location.reload();
    });

    socket.on('connect', () => {
        disconnected = false
    });

    socket.on('reconnecting', () => {
        createPersistentToast('Reconnecting!', 'Connection lost. Trying to reconnect to the server.', 'warning');
    });

    window.setInterval(pingAlive, 40000);

    function pingAlive() {
        socket.emit('alive_ping');
    }

    // BASE SOCKET MESSAGES
    socket.on('message', raw_data => {
        const data = cenOrOri(raw_data);
        if (data['state'] === 'join') {
            createToast('System says:', data['message'] + ' has joined!', 'info');
        }
        if (data['state'] === 'leave') {
            createToast('System says:', data['message']['username'] + ' has left!', 'info');
        }
        if (data['state'] === 'reload') {
            location.reload();
        }
        if (data['state'] === "notify") {
            createToast("System Says: ", data['message'], data['type']);
        }
        if (data['state'] === "not-in-room") {
            location.reload();
        }
        if (data['state'] === 'game-killed') {
            if (data['msg'] !== null) {
                createPersistentToast("System says: ", data['msg'], 'info');
                delayCallback(5000, function () {
                    window.location.replace("/server-browser");
                }).then();
            }

        }
    });
});

function flash(elementId) {
    const msg = $('#' + elementId);

    function animate() {
        msg.stop().fadeIn('fast', function () {
            msg.animate({opacity: 1}, 50, function () {
                msg.fadeOut('fast', function () {
                    $('#' + elementId).fadeIn('fast');
                })
            })
        });
    }

    animate();
}

function getPlayerData() {
    socket.emit('update')
}

function reloaded() {
    socket.emit('reloaded');
}

class Users {
    constructor() {
        this.listOfUsers = []
    }

    add(user) {
        this.listOfUsers.push(user)
    }

    remove(user) {
        for (let i = 0; i < this.listOfUsers.length; i++) {
            if (this.listOfUsers[i].equals(user.uuid)) {
                this.listOfUsers.splice(i, 1);
                return true;
            }
        }
        return false;
    }
}

class User {
    constructor(uuid, name, emoji, points, prev_points, streak, loaded, has_song, modified, guessed, disconnected) {
        this.uuid = uuid;
        this.display_name = name;
        this.emoji = emoji;
        this.points = points;
        this.prev_points = prev_points;
        this.streak = streak;
        this.song_loaded = loaded;
        this.has_song = has_song;
        this.modified = modified;
        this.guessed = guessed;
        this.disconnected = disconnected;
    }

    equals(otherUuid) {
        return otherUuid === this.uuid;
    }
}

function rebuildPlayerList(playerData) {
    if (!playerData)
        return;
    let newPlayerList = new Users();
    if (playerData.length === 0)
        return;
    gameState = playerData[0]['game_state'];
    for (let i = 0; i < playerData.length; i++) {
        const user = new User(
            playerData[i]['uuid'],
            playerData[i]['username'],
            playerData[i]['emoji'],
            playerData[i]['points'],
            playerData[i]['prev_points'],
            playerData[i]['streak'],
            playerData[i]['loaded'],
            playerData[i]['has_song'],
            playerData[i]['modified'],
            playerData[i]['guessed'],
            playerData[i]['disconnected']);
        newPlayerList.add(user);
    }
    players = newPlayerList;
}


let timeInOverlayInterval = null;
let timeInOverlay = 0;

function fancyTimeFormat(duration) {
    // Hours, minutes and seconds
    const hrs = ~~(duration / 3600);
    const mins = ~~((duration % 3600) / 60);
    const secs = ~~duration % 60;

    // Output like "1:01" or "4:03:59" or "123:03:59"
    let ret = "";

    if (hrs > 0) {
        ret += "" + hrs + "h " + (mins < 10 ? "0" : "");
    }

    if (mins > 0) {
        ret += "" + mins + "m " + (secs < 10 ? "0" : "");
    }

    ret += "" + secs + "s";
    return ret;
}

function countUp() {
    timeInOverlay += 1;


    const d = document.getElementById('loading-time');
    d.innerText = '  ' + fancyTimeFormat(timeInOverlay)
}

function enableOverlay(reason = 'Loading') {
    const d = document.getElementById('loading-message');
    d.innerText = reason + '...';
    timeInOverlayInterval = setInterval(countUp, 1000);
    $(".overlay").fadeIn(1000);
    $(".spinner").fadeIn(1000);
}

function disableOverlay() {
    clearInterval(timeInOverlayInterval);
    timeInOverlay = 0;
    const d = document.getElementById('loading-time');
    d.innerText = '  ' + fancyTimeFormat(timeInOverlay);
    $(".overlay").fadeOut(1000);
    $(".spinner").fadeOut(1000);
}

function fadeInPage() {
    if (!window.AnimationEvent) {
        return;
    }
    const fader = document.getElementById('fader');
    fader.classList.add('fade-out');
}

document.addEventListener('DOMContentLoaded', function () {
    if (!window.AnimationEvent) {
        return;
    }
    const anchors = document.getElementsByTagName('a');
    for (let idx = 0; idx < anchors.length; idx += 1) {
        if (anchors[idx].hostname !== window.location.hostname ||
            anchors[idx].pathname === window.location.pathname) {
            continue;
        }
        anchors[idx].addEventListener('click', function (event) {
            const fader = document.getElementById('fader'),
                anchor = event.currentTarget;
            const listener = function () {
                window.location = anchor.href;
                fader.removeEventListener('animationend', listener);
            };
            fader.addEventListener('animationend', listener);
            event.preventDefault();
            fader.classList.add('fade-in');
        });
    }
});

window.addEventListener('pageshow', function (event) {
    if (!event.persisted) {
        return;
    }
    const fader = document.getElementById('fader');
    fader.classList.remove('fade-in');
});

function screenWidth() {
    const width = window.innerWidth || document.documentElement.clientWidth || document.body.clientWidth;
    if (width >= 1600) {
        return 6;
    }
    if (width < 1600 && width >= 1400) {
        return 5;
    }
    if (width < 1400 && width >= 1200) {
        return 4;
    }
    if (width < 1200 && width >= 992) {
        return 3;
    }
    if (width < 992 && width >= 768) {
        return 2;
    }
    if (width < 768 && width >= 576) {
        return 1;
    }
    if (width < 576) {
        return 0;
    }

}

function createToast(title = "System Says: ", content, type = 'info') {
    // type = ['info', 'success', 'warning', 'error']
    const options = {
        "closeButton": true,
        "debug": false,
        "newestOnTop": true,
        "progressBar": true,
        "positionClass": "toast-top-right",
        "preventDuplicates": false,
        "onclick": null,
        "showDuration": "300",
        "hideDuration": "1000",
        "timeOut": "5000",
        "extendedTimeOut": "1000",
        "showEasing": "swing",
        "hideEasing": "linear",
        "showMethod": "fadeIn",
        "hideMethod": "fadeOut"
    };

    toastr[type](content, title, options);
}

function createPersistentToast(title = "System Says: ", content, type = 'info') {
    const options = {
        "closeButton": false,
        "debug": false,
        "newestOnTop": true,
        "progressBar": false,
        "positionClass": "toast-top-right",
        "preventDuplicates": true,
        "onclick": null,
        "showDuration": "300",
        "hideDuration": "1000",
        "timeOut": "0",
        "extendedTimeOut": "0",
        "showEasing": "swing",
        "hideEasing": "linear",
        "showMethod": "fadeIn",
        "hideMethod": "fadeOut"
    };

    toastr[type](content, title, options);
}

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function delayCallback(waitTime, callback) {
    await sleep(waitTime);
    callback();
}

async function delayCallbackArgs(waitTime, callback) {
    await sleep(waitTime);
    callback(arguments[2], arguments[3]);
}

let players = new Users();

function setCookie(cname, cvalue, exdays) {
    const d = new Date();
    d.setTime(d.getTime() + (exdays * 24 * 60 * 60 * 1000));
    const expires = "expires=" + d.toUTCString();
    document.cookie = cname + "=" + cvalue + ";" + expires + ";path=/";
}

function getCookie(cname) {
    const name = cname + "=";
    const decodedCookie = decodeURIComponent(document.cookie);
    const ca = decodedCookie.split(';');
    for (let i = 0; i < ca.length; i++) {
        let c = ca[i];
        while (c.charAt(0) === ' ') {
            c = c.substring(1);
        }
        if (c.indexOf(name) === 0) {
            return c.substring(name.length, c.length);
        }
    }
    return "";
}

function initNightmodeCookie() {
    let state = getCookie("nightmode");
    if (state !== "") {
        return state;
    } else { // if cookie doesnt exist set one
        state = 'false';
        if (state !== "" && state != null) {
            setCookie("nightmode", state, 365);
        }
    }
}

function initVisualizerCookie() {
    let state = getCookie("visualizer");
    if (state !== "") {
        return state;
    } else { // if cookie doesnt exist set one
        state = 'random';
        if (state !== "" && state != null) {
            setCookie("visualizer", state, 365);
        }
    }
}

