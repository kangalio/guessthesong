let sound = null;
let newSound = null; // used for preloading new sound
let songId = 0;
let mute = false;
let drawVisual = null; // Animation id
let visualizer_color_buffer = []; //this buffer contains all colours currently in the mix, if its len is 0 it defaults to `visualizer_color`
let array_of_already_won_uuids = [];
let someoneGuessedFrameCount = 0; // this is the number of frames that the eye should remain closed. 0 means open, >0 means close for that many frames.
const BASE_AMPLITUDE_SCALAR = 1.25;
const PARTICLES = [];
const NO_OF_PARTICLES = 1500;
const MAX_PARTICLE_SIZE = 1;
const SHAKE_THRESHOLD_DAMPER = 0.15; // constant in relation to amplitude
let AMPLITUDE_OVER_TIME = [];


function loadAudio(player_uuid, room_code, playFailCallback, fallback = true) {
    stopAudio();
    const slider = document.getElementById("myRange");
    let vol;
    if (slider) {
        vol = (slider.value / 100);
    } else {
        vol = 0.5
    }

    const width = window.innerWidth || document.documentElement.clientWidth || document.body.clientWidth;
    if (width < 768) {
        vol = 1;
    }

    // Trigger volume icon to be rendered
    renderVolumeIcon();

    newSound = new Howl({
        src: ['/song/' + player_uuid + '/' + room_code + '/' + Math.floor(Math.random() * 10000)],
        format: ['mp3'],
        autoplay: false,
        loop: true,
        volume: vol,
        preload: true,
        onload: function () {
            socket.send(JSON.stringify({
                'type': 'audio-loaded',
            }));
        },
        onloaderror: function (id, err) {
            console.log(err)
        },
    });

    if (fallback) {
        delayCallback(500, function () {
            if (Howler.ctx.state === "suspended") {
                playFailCallback(false);
            }
        }).then();
    }
}


function playAudio(time = 0, roundTime) {
    if (sound !== null) {
        if (sound.playing(songId)) {
            return;
        }
    }
    sound = newSound;

    if (sound == null)
        return;

    songId = sound.play();
    if (time !== 0) {
        sound.seek(roundTime - time);
        move(roundTime - time, roundTime);
    } else {
        move(0, roundTime);
    }


    sound.fade(0, (slider.value / 100), 2000, songId);
    visualizer_controller();
}

function visualizer_controller() {
    initVisualizerCookie();
    // Create an analyser node in the Howler WebAudio context
    const analyser = Howler.ctx.createAnalyser();
    // Connect the masterGain -> analyser (disconnecting masterGain -> destination)
    Howler.masterGain.connect(analyser);

    if (getCookie("visualizer") === 'random') {
        const ran = Math.floor(Math.random() * 2);
        if (ran === 0) {
            draw_circle_visualizer(analyser);
        } else {
            draw_bar_visualizer(analyser);
        }
    }
    if (getCookie("visualizer") === 'circle') {
        draw_circle_visualizer(analyser);
    }
    if (getCookie("visualizer") === 'bar') {
        draw_bar_visualizer(analyser);
    }
}

function getAverageArray(array, divisor = 2) {
    const len = array.length;
    const newLen = Math.round(len / divisor);
    const newArray = [];
    const averageElementIndex = len / newLen;
    for (let i = 0; i < len; i += averageElementIndex) {
        let avg = 0;
        for (let j = 0; j < averageElementIndex; j++) {
            avg += array[i + j]
        }
        newArray.push(avg / averageElementIndex)
    }

    return new Uint8Array(newArray);
}

function getDivisor() {
    const width = window.innerWidth || document.documentElement.clientWidth || document.body.clientWidth;
    if (width < 1350)
        return 4;
    if (width < 575)
        return 8;
    return 2;
}

function getBarHeightModifier() {
    switch (screenWidth()) {
        case 0:
            return 3;
        case 1:
            return 3;
        case 2:
            return 3;
        case 3:
            return 2;
        case 4:
            return 2;
        case 5:
            return 1.25;
        case 6:
            return 1;
    }
}

function getShakeModifier() {
    switch (screenWidth()) {
        case 0:
            return 0.6;
        case 1:
            return 0.7;
        case 2:
            return 0.7;
        case 3:
            return 0.7;
        case 4:
            return 0.8;
        case 5:
            return 0.8;
        case 6:
            return 1;
    }
}

function draw_bar_visualizer(analyser) {
    const canvas = document.getElementById('canvas');
    const canvasCtx = canvas.getContext('2d');

    analyser.fftSize = 256;
    const bufferLength = analyser.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);
    let reducedArray = [];

    canvasCtx.clearRect(0, 0, canvas.width, canvas.height);
    resizeCanvas(canvas);
    let divisor = getDivisor();


    function draw() {
        drawVisual = requestAnimationFrame(draw);
        analyser.getByteFrequencyData(dataArray);
        reducedArray = getAverageArray(dataArray.slice(0, 80), divisor);

        canvasCtx.fillStyle = "#343a40";
        canvasCtx.fillRect(0, 0, canvas.width, canvas.height);

        const barWidth = ((canvas.width - reducedArray.length) / (reducedArray.length));
        let barHeight;
        let x = 0;
        for (let i = 0; i < reducedArray.length; i++) {
            barHeight = Math.max(reducedArray[i] - (6 * (divisor - 2)), 0);
            canvasCtx.fillStyle = rainbow(i * (divisor + 1), Math.min(normal(barHeight, canvas.height, 0) + 0.2, 1.0));
            canvasCtx.fillRect(x, canvas.height - barHeight, barWidth, barHeight + barWidth);
            x += barWidth + 1;
        }
    }

    draw();
}


function draw_circle_visualizer(analyser) {
    analyser.fftSize = 2048;
    const bufferLength = analyser.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);
    const canvas = document.getElementById('canvas');
    const canvasCtx = canvas.getContext('2d');
    AMPLITUDE_OVER_TIME = []; //empty array;
    array_of_already_won_uuids = [];


    canvasCtx.clearRect(0, 0, canvas.width, canvas.height);
    canvasCtx.imageSmoothingEnabled = false;
    resizeCanvas(canvas);
    const img = new Image;
    if (screenWidth() < 5)
        img.src = logoAddressSmall;
    else
        img.src = logoAddress;

    const img_blink = new Image;
    if (screenWidth() < 5)
        img_blink.src = logoAddressSmallClosedEye;
    else
        img_blink.src = logoAddressClosedEye;

    function draw() {
        drawVisual = requestAnimationFrame(draw);
        analyser.getByteFrequencyData(dataArray);
        canvasCtx.fillStyle = "#343a40";
        canvasCtx.fillRect(0, 0, canvas.width, canvas.height);

        canvasCtx.lineWidth = 2;
        canvasCtx.strokeStyle = "#FFFFFF";
        // tiled array, normalized, then mirrored twice
        let musicArray = tiltCircle(normalize(createPerfectLoopArray(createPerfectLoopArray(getAverageArray(dataArray.slice(0, 640), 8))), 100, 5), 8);
        let barHeight;
        const w = canvas.width;
        const h = canvas.height;
        const r = h / 3;

        const angleUnit = Math.PI * 2 / musicArray.length;
        let angle = 0;
        let x0 = 0, y0 = 0;
        let imageWidth = r;
        let imageHeight = r;
        let amplitude = getArrayAverageAmplitude(dataArray.slice(0, 640));

        //only store last 30 seconds.
        if (AMPLITUDE_OVER_TIME.length >= (30 * 60)) {
            AMPLITUDE_OVER_TIME.shift();
        }
        AMPLITUDE_OVER_TIME.push(amplitude);

        canvasCtx.clearRect(0, 0, canvas.width, canvas.height);
        emitter(amplitude);
        renderer();
        preShake(amplitude * getShakeModifier());
        if (someoneGuessedFrameCount > 0) {
            canvasCtx.drawImage(img_blink, (w / 2) - (imageWidth * amplitude / 2), (h / 2) - (imageHeight * amplitude / 2), imageWidth * amplitude, imageHeight * amplitude);
            someoneGuessedFrameCount--;
        } else {
            canvasCtx.drawImage(img, (w / 2) - (imageWidth * amplitude / 2), (h / 2) - (imageHeight * amplitude / 2), imageWidth * amplitude, imageHeight * amplitude);
        }
        postShake();


        for (let i = 0; i < musicArray.length + 1; i++) {
            barHeight = musicArray[i];
            if (screen.height > screen.width) {
                barHeight /= 3;
            } else {
                barHeight /= getBarHeightModifier();
            }
            const {x, y} = randCoordinate(w, h, r + barHeight, angle);

            if (i !== 0) { // first node doesnt have anything to attach to yet
                const {x: xC, y: yC} = randCoordinate(w, h, r + barHeight, angle - angleUnit / 2);
                canvasCtx.beginPath();
                canvasCtx.moveTo(x0, y0);
                canvasCtx.quadraticCurveTo(xC, yC, x, y);
                canvasCtx.stroke();
            }
            if (i === musicArray.length) {
                barHeight = musicArray[musicArray.length];
                if (screen.height > screen.width) {
                    barHeight /= 3;
                } else {
                    barHeight /= getBarHeightModifier();
                }
                let angle = 0;
                const {x, y} = randCoordinate(w, h, r + barHeight, angle);
                const {x: xC, y: yC} = randCoordinate(w, h, r + barHeight, angle - angleUnit / 2);
                canvasCtx.moveTo(x0, y0);
                canvasCtx.quadraticCurveTo(xC, yC, x, y);
                canvasCtx.stroke();
            }
            angle += angleUnit;
            x0 = x;
            y0 = y;
        }
    }

    function particle(amplitude, particleSize = 2.5) {
        // Create a new particle
        amplitude = amplitude - BASE_AMPLITUDE_SCALAR;
        let myColor = "#FFFFFF";

        if (visualizer_color_buffer.length > 0) {
            let index = Math.floor((Math.random() * visualizer_color_buffer.length));
            myColor = visualizer_color_buffer[index].color;
            visualizer_color_buffer[index].duration--;
            if (visualizer_color_buffer[index].duration <= 0) {
                visualizer_color_buffer.splice(index, 1);
            }
        }


        const id = PARTICLES.length,
            x = canvas.width / 2,
            y = canvas.height / 2,
            vx = (Math.random() * 2 - 1) * amplitude * 10,
            vy = (Math.random() * 2 - 1) * amplitude * 10,
            size = Math.floor((Math.random() * MAX_PARTICLE_SIZE) + particleSize),
            life = 0,
            death = Math.random() * 400 - 5;
        PARTICLES[PARTICLES.length] = {
            'id': id,
            'x': x,
            'y': y,
            'vx': vx,
            'vy': vy,
            'size': size,
            'life': life,
            'death': death,
            'color': myColor
        };
    }

    function emitter(amplitude) {
        // Update the particles
        for (let i = 0; i < PARTICLES.length; i++) {
            if (amplitude >= get_array_average(AMPLITUDE_OVER_TIME) + SHAKE_THRESHOLD_DAMPER) {
                PARTICLES[i].x += PARTICLES[i].vx * 2 * amplitude;
                PARTICLES[i].y += PARTICLES[i].vy * 2 * amplitude;

            } else {
                PARTICLES[i].x += PARTICLES[i].vx;
                PARTICLES[i].y += PARTICLES[i].vy;
            }

            PARTICLES[i].life++;
            PARTICLES[i].size += 0.01;

            // Remove dead particles
            if (PARTICLES[i].life > PARTICLES[i].death) {
                PARTICLES.splice(i, 1);
            }
        }

        // Create new particles
        while (PARTICLES.length < NO_OF_PARTICLES) {
            particle(amplitude);
        }
    }

    function renderer() {
        for (let i = 0; i < PARTICLES.length; i++) {
            canvasCtx.fillRect(PARTICLES[i].x, PARTICLES[i].y, PARTICLES[i].size, PARTICLES[i].size);
            canvasCtx.fillStyle = PARTICLES[i].color;
        }
    }

    function randCoordinate(w, h, r, angle) { // generate radian coordinates
        return {
            x: r * Math.cos(angle) + w / 2,
            y: r * Math.sin(angle) + h / 2
        };
    }

    function preShake(amplitude, multiplier = 2) {
        canvasCtx.save();
        if (amplitude >= get_array_average(AMPLITUDE_OVER_TIME) + SHAKE_THRESHOLD_DAMPER) {
            const dx = Math.random() * amplitude * multiplier ** 2;
            const dy = Math.random() * amplitude * multiplier ** 2;
            canvasCtx.translate(dx, dy);
        }
    }

    function postShake() {
        canvasCtx.restore();
    }

    draw();

}

function get_array_average(myArray) {
    let total = 0;
    for (let i = 0; i < myArray.length; i++) {
        total += myArray[i];
    }
    return Math.round(((total / myArray.length) + Number.EPSILON) * 100) / 100; // apparently this garbage is required in js to round to 2 decimals.
}

function getArrayAverageAmplitude(array) { // get int representing avg amplitude of the music with a base of 1.
    let total = 0;
    for (let i = 0; i < array.length; i++) {
        total += array[i];
    }
    let avg = total / array.length;
    return BASE_AMPLITUDE_SCALAR + (Math.ceil(avg) / 100);
}

function tiltCircle(arr, tiltAmount = 25) {// tilt, done by shifting the array.
    let newArray = [...arr];
    for (let i = 0; i < tiltAmount; i++) {
        newArray.push(arr[i])
    }
    newArray = newArray.slice(tiltAmount - 1, newArray.length);
    return new Uint8Array(newArray);
}

// makes everything smaller than the biggest
function normalize(arr, amplitude = 150, damper = 5) {
    const newArray = [];
    const largestNumber = Math.max(...arr);
    for (let i = 0; i < arr.length; i++) {
        if (arr[i] < (largestNumber / (damper))) {
            newArray.push(arr[i] / (largestNumber / (damper)));
        } else {
            newArray.push((arr[i] / largestNumber) * amplitude);
        }
    }
    return new Uint8Array(newArray);

}

function createPerfectLoopArray(arr) {// mirror array
    const newArray = [...arr];
    arr = arr.reverse();
    for (let i = 0; i < arr.length; i++) {
        newArray.push(arr[i])
    }
    return new Uint8Array(newArray);
}

document.addEventListener("DOMContentLoaded", function () {
    resizeCanvas(document.getElementById("canvas"));
});

window.addEventListener('resize', function () {
    resizeCanvas(document.getElementById("canvas"));
});

function resizeCanvas(canvas) {
    canvas.width = canvas.parentElement.clientWidth;
    canvas.height = canvas.parentElement.clientHeight;
}

function stopAudio(fade = true) {
    if (sound != null) {
        if (fade) {
            sound.fade((slider.value / 100), 0, 5000, songId);
            delayCallback(5000, function () {
                if (sound != null)
                    sound.stop();
            }).then();
            delayCallback(6000, function () {
                if (drawVisual != null) {
                    window.cancelAnimationFrame(drawVisual);
                }
            }).then();
        } else {
            sound.stop();
            delayCallback(2000, function () {
                if (drawVisual != null) {
                    window.cancelAnimationFrame(drawVisual);
                }
            }).then();
        }
        clearInterval(timerId);
        time = 0;
        return true;
    } else {
        return false;
    }
}

function changeVolume(fol) {
    if (sound != null) {
        sound.volume(fol, songId);

        if (fol === 0) {
            setVolumeIconOff()
        } else if (fol > 0 && mute) {
            setVolumeIconOn()
        }
    }
}

function setVolumeIconOff() {
    const volumeIcon = document.getElementsByClassName('volume-icon')[0];

    mute = true;
    const icon = document.createElement('i');
    icon.className = 'fas fa-volume-off';
    icon.style.marginRight = '6px';
    icon.style.marginLeft = '6px';
    volumeIcon.innerHTML = '';
    volumeIcon.appendChild(icon);
}

function setVolumeIconOn() {
    const volumeIcon = document.getElementsByClassName('volume-icon')[0];

    const icon = document.createElement('i');
    icon.className = 'fas fa-volume-up';
    volumeIcon.innerHTML = '';
    volumeIcon.appendChild(icon);
    mute = false;
}

function renderVolumeIcon() {
    const fol = document.getElementById('myRange').value / 100;

    if (fol === 0) {
        setVolumeIconOff()
    } else if (fol > 0) {
        setVolumeIconOn()
    }
}

function duration() {
    if (sound != null) {
        sound.duration(songId)
    }
}

let time = 0;
let timerId = null;

function move(startTime = 0, roundTime) {
    if (time === 0) {
        time = 1;
        let counter = 0;
        const elem = document.getElementById("myBar");

        const widthIncrement = (100 / ((roundTime * 1000) / 51));
        let width = ((startTime * 1000) / 50) * widthIncrement;
        timerId = setInterval(frame, 50);

        function frame() {
            counter += 0.05;
            if (width >= 100) {
                clearInterval(timerId);
                time = 0;
            } else {
                if (Math.round(counter) % 2 === 0) {
                    if (Math.round(counter) % 4 === 0) {
                        $('#sandtime').addClass('animated');
                    } else {
                        $('#sandtime').removeClass('animated');
                    }
                }
                width += widthIncrement;
                elem.style.width = width + "%";
                const rgb = hslToRgb(normal(181, 360, 0),
                    Math.min(100, Math.round(width / 2) + 50) / 100,
                    Math.min(75, Math.round(width / 2) + 45) / 100);
                elem.style.backgroundColor = 'rgb(' + rgb[0] + ',' + rgb[1] + ',' + rgb[2] + ')';
            }
        }
    }
}

function normal(val, max, min) {
    return (val - min) / (max - min);
}


function rgbToHsl(r, g, b) {
    r /= 255;
    g /= 255;
    b /= 255;
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    let h, s, l = (max + min) / 2;

    if (max === min) {
        h = s = 0; // achromatic
    } else {
        const d = max - min;
        s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
        switch (max) {
            case r:
                h = (g - b) / d + (g < b ? 6 : 0);
                break;
            case g:
                h = (b - r) / d + 2;
                break;
            case b:
                h = (r - g) / d + 4;
                break;
        }
        h /= 6;
    }

    return [h, s, l];
}


/*
 * P: color value
 * Saturation: 0.0 to 1.0
 */
function rainbow(p, saturation) {
    const rgb = HSVtoRGB(p / 100.0 * 0.85, saturation, 1.0);
    return 'rgb(' + rgb.r + ',' + rgb.g + ',' + rgb.b + ')';
}

function idToRgb(id) {
    return {
        r: id & 0xFF,
        g: (id >> 8) & 0xFF,
        b: (id >> 16) & 0xFF,
    };
}

function rgbToHex(rgb) {
    let a = rgb.split("(")[1].split(")")[0];

    a = a.split(",");

    let b = a.map(function (x) {             //For each array element
        x = parseInt(x).toString(16);      //Convert to a base16 string
        x = x.toUpperCase();
        return (x.length === 1) ? "0" + x : x;  //Add zero if we get only one character
    });

    b = "#" + b.join("");
    return b;
}


/**
 * Converts an HSL color value to RGB. Conversion formula
 * adapted from http://en.wikipedia.org/wiki/HSL_color_space.
 * Assumes h, s, and l are contained in the set [0, 1] and
 * returns r, g, and b in the set [0, 255].
 *
 * @param   {number}  h       The hue
 * @param   {number}  s       The saturation
 * @param   {number}  l       The lightness
 * @return  {Array}           The RGB representation
 */
function hslToRgb(h, s, l) {
    let r, g, b;

    if (s === 0) {
        r = g = b = l; // achromatic
    } else {
        const hue2rgb = function hue2rgb(p, q, t) {
            if (t < 0) t += 1;
            if (t > 1) t -= 1;
            if (t < 1 / 6) return p + (q - p) * 6 * t;
            if (t < 1 / 2) return q;
            if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
            return p;
        };

        const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
        const p = 2 * l - q;
        r = hue2rgb(p, q, h + 1 / 3);
        g = hue2rgb(p, q, h);
        b = hue2rgb(p, q, h - 1 / 3);
    }

    return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255)];
}


/* accepts parameters
 * h  Object = {h:x, s:y, v:z}
 * OR
 * h, s, v
*/
function HSVtoRGB(h, s, v) {
    let r, g, b, i, f, p, q, t;
    if (arguments.length === 1) {
        s = h.s;
        v = h.v;
        h = h.h;
    }
    i = Math.floor(h * 6);
    f = h * 6 - i;
    p = v * (1 - s);
    q = v * (1 - f * s);
    t = v * (1 - (1 - f) * s);
    switch (i % 6) {
        case 0:
            r = v;
            g = t;
            b = p;
            break;
        case 1:
            r = q;
            g = v;
            b = p;
            break;
        case 2:
            r = p;
            g = v;
            b = t;
            break;
        case 3:
            r = p;
            g = q;
            b = v;
            break;
        case 4:
            r = t;
            g = p;
            b = v;
            break;
        case 5:
            r = v;
            g = p;
            b = q;
            break;
    }
    return {
        r: Math.round(r * 255),
        g: Math.round(g * 255),
        b: Math.round(b * 255)
    };
}

