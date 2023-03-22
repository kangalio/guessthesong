document.addEventListener('DOMContentLoaded', () => {

    const left = document.getElementById('emoji-choice-left');
    const right = document.getElementById('emoji-choice-right');
    const preview = document.getElementById('ePreview');
    const previewLeft = document.getElementById('ePreviewLeft');
    const previewRight = document.getElementById('ePreviewRight');

    let currentIndex = Math.floor(Math.random() * emojiList.length);
    const storedValue = getCookie('emoji');
    if (storedValue === '') {
        setPreview(currentIndex);
    } else {
        currentIndex = parseInt(storedValue);
        setPreview(currentIndex);
    }

    function setPreview(index) {
        preview.innerText = emojiList[index];
        if (previewRight && previewLeft) {
            previewLeft.innerText = emojiList[((index - 1 >= 0) ? index - 1 : emojiList.length - 1)];
            previewRight.innerText = emojiList[((index + 1 < emojiList.length) ? index + 1 : 0)];
        }
        storeChoice(index);
    }

    function setRight() {
        if (currentIndex + 1 >= emojiList.length)
            currentIndex = 0;
        else
            currentIndex += 1;
        setPreview(currentIndex)
    }

    function setLeft() {
        // const leftPreview = document.getElementById('emoji-choice-left');
        // leftPreview.style.right = '201px';
        // leftPreview.style.opacity = '0.6';

        if (currentIndex - 1 < 0)
            currentIndex = emojiList.length - 1;
        else
            currentIndex -= 1;
        setPreview(currentIndex)
    }

    function storeChoice(index) {
        setCookie('emoji', index, 365);
    }

    left.addEventListener('click', function () {
        setLeft();

        // $("#emoji-choice-left").animate({
        //     right: '250px',
        //     opacity: '0'
        // }, 1000, setLeft);
        //
        // $("#emoji-choice-main").animate({
        //     right: '100px',
        //     width: '100px',
        //     opacity: 0.6,
        //     fontSize: '45px'
        // }, 1000);
        // $("#emoji-choice-A").animate({
        //     height: '90px'
        // }, 1000);
        //
        //
        // $("#emoji-choice-right").animate({
        //     left: '50px',
        //     opacity: 1,
        //     width: '200px'
        // }, 1000);
        // $("#emoji-choice-rightA").animate({
        //     height: '100px'
        // }, 1000);
    });

    function resetMainPreview() {
        const mainPreview = document.getElementById('emoji-choice-main');
        mainPreview.style.right = '0';
        mainPreview.style.width = '100%';
        mainPreview.style.opacity = '1';
    }

    function resetRightPreview() {
        const rightPreview = document.getElementById('emoji-choice-right');
        rightPreview.style.left = '201px';
        rightPreview.style.opacity = '0.6';
    }

    right.addEventListener('click', function () {
        setRight();
    });

    window.onbeforeunload = function () {
        if (authenticated === true) {
            $.post(account_settings_url,
                {
                    data: JSON.stringify({
                        'emoji': currentIndex,
                        'nightmode': null,
                        'censor': null,
                        'visualizer': null
                    })
                })
        }
    };
});
