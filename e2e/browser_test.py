"""Drives the real UI in headless Firefox, captures browser console output and JS errors
(WebDriver BiDi), checks responsive layout, and saves screenshots. Normally run by e2e/run.sh
against a freshly started fake GitHub (expects the e2e-user account with no stored data).

Env: E2E_WEB_URL, E2E_ARTIFACTS (screenshots), GECKODRIVER (path, default: from PATH).
Exit status is non-zero on any failed step, console error/warning or uncaught JS error.
"""
import os
import shutil
import sys
import time

from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.firefox.service import Service
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import Select, WebDriverWait

WEB = os.environ.get("E2E_WEB_URL", "http://localhost:13000")
OUT = os.environ.get("E2E_ARTIFACTS", os.path.join(os.path.dirname(os.path.abspath(__file__)), "artifacts"))
GECKODRIVER = os.environ.get("GECKODRIVER") or shutil.which("geckodriver")
if not GECKODRIVER:
    sys.exit("geckodriver not found: install Firefox + geckodriver or set GECKODRIVER")
os.makedirs(OUT, exist_ok=True)

opts = webdriver.FirefoxOptions()
opts.add_argument("-headless")
opts.enable_bidi = True
driver = webdriver.Firefox(options=opts, service=Service(executable_path=GECKODRIVER))
driver.set_window_size(1280, 900)
wait = WebDriverWait(driver, 20)

console, js_errors = [], []
driver.script.add_console_message_handler(lambda m: console.append((m.level, m.text)))
driver.script.add_javascript_error_handler(lambda e: js_errors.append(e.text))

results = []
created_issue_path = []


def step(name, fn):
    try:
        fn()
        results.append((True, name))
        print(f"  ✓ {name}")
    except Exception as e:  # noqa: BLE001
        results.append((False, name))
        print(f"  ✗ {name}: {type(e).__name__}: {str(e).splitlines()[0] if str(e) else ''}")
        driver.save_screenshot(os.path.join(OUT, f"FAIL-{len(results)}.png"))


def text_present(text, timeout=20):
    WebDriverWait(driver, timeout).until(EC.text_to_be_present_in_element((By.TAG_NAME, "body"), text))


def click_button(label):
    btn = wait.until(EC.element_to_be_clickable((By.XPATH, f"//button[normalize-space()='{label}']")))
    btn.click()


def shot(name):
    driver.save_screenshot(os.path.join(OUT, f"{name}.png"))


def login():
    driver.get(f"{WEB}/")
    text_present("Sign in with GitHub")
    assert "/login" in driver.current_url, driver.current_url
    shot("01-login")
    click_button("Sign in with GitHub")
    text_present("Dashboard")
    text_present("You are not tracking any repositories yet")
    shot("02-dashboard-empty")


def track_repo():
    driver.find_element(By.LINK_TEXT, "Repositories").click()
    text_present("e2e-user/demo")
    shot("03-repositories")
    first = driver.find_elements(By.XPATH, "//li[.//a[normalize-space()='e2e-user/demo']]")[0]
    first.find_element(By.XPATH, ".//button[normalize-space()='Track']").click()
    text_present("Synced 4 issues")


def browse_issues():
    driver.find_element(By.LINK_TEXT, "e2e-user/demo").click()
    text_present("App crashes on start")
    shot("04-issues")
    driver.find_element(By.XPATH, "//button[normalize-space()='closed']").click()
    text_present("Old closed bug")
    assert "state=closed" in driver.current_url
    Select(driver.find_element(By.XPATH, "//label[contains(., 'Label')]/select")).select_by_visible_text("bug")
    WebDriverWait(driver, 10).until(lambda d: "label=bug" in d.current_url)
    click_button("Clear filters")
    box = driver.find_element(By.ID, "issue-search")
    box.send_keys("dark")
    click_button("Search")
    text_present("1 matching issues")
    text_present("Add dark mode")


def create_issue():
    driver.find_element(By.LINK_TEXT, "New issue").click()
    click_button("Create issue")
    text_present("Title is required.")
    driver.find_element(By.ID, "title").send_keys("Created from the browser")
    driver.find_element(By.ID, "body").send_keys("Line one\nLine two")
    driver.find_element(By.XPATH, "//label[.//span[normalize-space()='bug']]/input").click()
    shot("05-new-issue")
    click_button("Create issue")
    text_present("Issue created.")
    text_present("Created from the browser")
    created_issue_path.append(driver.current_url.replace(WEB, "").split("?")[0])


def edit_issue():
    click_button("Edit title")
    t = driver.find_element(By.ID, "edit-title")
    t.clear()
    t.send_keys("Edited in the browser")
    click_button("Save")
    text_present("Changes saved to GitHub.")
    text_present("Edited in the browser")
    # assign helper
    driver.find_element(By.XPATH, "//section[@aria-label='Assignees']//button[normalize-space()='Edit']").click()
    driver.find_element(By.XPATH, "//section[@aria-label='Assignees']//label[contains(., 'helper')]/input").click()
    driver.find_element(By.XPATH, "//section[@aria-label='Assignees']//button[normalize-space()='Save']").click()
    WebDriverWait(driver, 10).until(
        lambda d: "helper" in d.find_element(By.XPATH, "//section[@aria-label='Assignees']").text
        and "Save" not in d.find_element(By.XPATH, "//section[@aria-label='Assignees']").text
    )
    # labels
    Select(driver.find_element(By.ID, "add-label")).select_by_visible_text("enhancement")
    driver.find_element(By.XPATH, "//section[@aria-label='Labels']//button[normalize-space()='Add']").click()
    text_present("Labels updated on GitHub.")
    driver.find_element(By.XPATH, "//button[@aria-label='Remove label bug']").click()
    WebDriverWait(driver, 10).until(lambda d: not d.find_elements(By.XPATH, "//button[@aria-label='Remove label bug']"))
    # comment
    click_button("Comment")
    text_present("Write a comment first.")
    driver.find_element(By.ID, "new-comment").send_keys("Comment from the browser")
    click_button("Comment")
    WebDriverWait(driver, 10).until(lambda d: "Comment from the browser" in d.find_element(By.XPATH, "//section[@aria-label='Comments']//ul").text)
    # close & reopen
    click_button("Close as not planned")
    text_present("Closed (not planned)")
    click_button("Reopen issue")
    WebDriverWait(driver, 10).until(lambda d: d.find_elements(By.XPATH, "//button[normalize-space()='Close as completed']"))
    shot("06-issue-detail")


def dashboard():
    driver.find_element(By.LINK_TEXT, "Dashboard").click()
    text_present("Top labels on open issues")
    text_present("Edited in the browser")
    shot("07-dashboard")


def responsive():
    driver.set_window_size(390, 844)
    for path in ["/", "/repos", "/repos/e2e-user/demo", *created_issue_path, "/account"]:
        driver.get(WEB + path)
        time.sleep(1.5)
        sw, iw = driver.execute_script("return [document.documentElement.scrollWidth, window.innerWidth]")
        assert sw <= iw, f"{path}: horizontal overflow {sw} > {iw}"
    driver.find_element(By.XPATH, "//button[@aria-label='Open menu']").click()
    text_present("Repositories")
    shot("08-mobile-menu")
    driver.set_window_size(1280, 900)


def account_signout():
    driver.get(WEB + "/account")
    text_present("Disconnect GitHub account")
    click_button("Sign out")
    text_present("Sign in with GitHub")


for name, fn in [
    ("redirect to login, sign in with GitHub, empty dashboard", login),
    ("load repositories, track + sync a repository", track_repo),
    ("issue list: state/label filters, clear, search", browse_issues),
    ("new issue: validation error, then create", create_issue),
    ("detail: edit title, assign, add/remove label, comment, close, reopen", edit_issue),
    ("dashboard reflects changes", dashboard),
    ("responsive at 390px: no horizontal overflow, mobile menu works", responsive),
    ("sign out", account_signout),
]:
    step(name, fn)

time.sleep(1)
driver.quit()

problems = [(lvl, t) for lvl, t in console if lvl in ("error", "warn", "warning")]
print(f"\nconsole messages: {len(console)} total, {len(problems)} warnings/errors, {len(js_errors)} uncaught JS errors")
for lvl, t in problems + [("uncaught", t) for t in js_errors]:
    print(f"  [{lvl}] {t[:300]}")
ok = all(r for r, _ in results)
print(f"\n{sum(r for r, _ in results)}/{len(results)} browser steps passed")
sys.exit(0 if ok and not js_errors and not problems else 1)
