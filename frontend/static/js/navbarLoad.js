// There is some weird behavior with loading the navbar. This code waits till the page is loaded and then displays
// the elements.
document.addEventListener('DOMContentLoaded', () => {
    const elements = document.getElementsByClassName('nav-item');
    for (let i = 0; i < elements.length; i++) {
        elements[i].style.display = 'block';
    }

    const seenVersion = getCookie('seenVersion');
    if (seenVersion === '' || seenVersion !== version) {
        document.getElementById('updatesNotification').innerHTML = '1';
        document.getElementById('updatesNotification2').innerHTML = '1';
    }

    const userIcon = document.getElementById("user-icon");
    if (userIcon) {
        const storedValue = getCookie('emoji');
        if (storedValue === '') {
            userIcon.innerText = '<i class="far fa-user-circle"></i>'
        } else {
            userIcon.innerText = emojiList[parseInt(storedValue)];
        }
    }
});
