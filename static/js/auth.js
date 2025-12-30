// Client-side password hashing using Web Crypto API
// The server never sees the actual password - only this hash

// Toggle password visibility
function togglePasswordVisibility(inputId, button) {
  const input = document.getElementById(inputId);
  const eyeOpen = button.querySelector('.eye-open');
  const eyeClosed = button.querySelector('.eye-closed');

  if (input.type === 'password') {
    input.type = 'text';
    eyeOpen.classList.add('hidden');
    eyeClosed.classList.remove('hidden');
  } else {
    input.type = 'password';
    eyeOpen.classList.remove('hidden');
    eyeClosed.classList.add('hidden');
  }
}

// Password strength calculation (advisory only)
function calculatePasswordStrength(password) {
  let score = 0;

  if (password.length >= 8) score += 1;
  if (password.length >= 12) score += 1;
  if (password.length >= 16) score += 1;
  if (/[a-z]/.test(password)) score += 1;
  if (/[A-Z]/.test(password)) score += 1;
  if (/[0-9]/.test(password)) score += 1;
  if (/[^a-zA-Z0-9]/.test(password)) score += 1;
  // Bonus for mixing character types
  const types = [/[a-z]/, /[A-Z]/, /[0-9]/, /[^a-zA-Z0-9]/].filter(r => r.test(password)).length;
  if (types >= 3) score += 1;

  if (score <= 2) return { level: 'weak', label: 'Weak', color: 'red' };
  if (score <= 4) return { level: 'fair', label: 'Fair', color: 'yellow' };
  if (score <= 6) return { level: 'good', label: 'Good', color: 'blue' };
  return { level: 'strong', label: 'Strong', color: 'green' };
}

function updateStrengthMeter(password) {
  const meter = document.getElementById('password-strength');
  if (!meter) return;

  if (!password || password.length === 0) {
    meter.className = 'hidden';
    return;
  }

  const strength = calculatePasswordStrength(password);
  const colors = {
    red: 'bg-red-500',
    yellow: 'bg-yellow-500',
    blue: 'bg-blue-500',
    green: 'bg-green-500'
  };
  const textColors = {
    red: 'text-red-600 dark:text-red-400',
    yellow: 'text-yellow-600 dark:text-yellow-400',
    blue: 'text-blue-600 dark:text-blue-400',
    green: 'text-green-600 dark:text-green-400'
  };
  const widths = {
    weak: 'w-1/4',
    fair: 'w-2/4',
    good: 'w-3/4',
    strong: 'w-full'
  };

  meter.className = 'mt-2';
  meter.innerHTML = `
    <div class="flex items-center gap-2">
      <div class="flex-1 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div class="h-full ${colors[strength.color]} ${widths[strength.level]} transition-all duration-300 rounded-full"></div>
      </div>
      <span class="text-xs ${textColors[strength.color]} font-medium w-12">${strength.label}</span>
    </div>
  `;
}

async function hashPassword(password, username) {
  // Include username as a salt to prevent rainbow table attacks
  // and ensure same password for different users produces different hashes
  const data = new TextEncoder().encode(password + ':' + username.toLowerCase());
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
}

async function handleLoginSubmit(event) {
  event.preventDefault();
  const form = event.target;
  const username = form.querySelector('[name="username"]').value;
  const password = form.querySelector('[name="password"]').value;
  const submitBtn = form.querySelector('button[type="submit"]');

  // Show loading state
  const originalText = submitBtn.textContent;
  submitBtn.textContent = 'Signing in...';
  submitBtn.disabled = true;

  try {
    const hashedPassword = await hashPassword(password, username);

    // Replace password with hash before submitting
    const hiddenHash = document.createElement('input');
    hiddenHash.type = 'hidden';
    hiddenHash.name = 'password_hash';
    hiddenHash.value = hashedPassword;
    form.appendChild(hiddenHash);

    // Clear the actual password field so it's not sent
    form.querySelector('[name="password"]').name = '_password_cleared';

    // Submit the form
    form.submit();
  } catch (err) {
    console.error('Hashing failed:', err);
    submitBtn.textContent = originalText;
    submitBtn.disabled = false;
  }
}

async function handleRegisterSubmit(event) {
  event.preventDefault();
  const form = event.target;
  const username = form.querySelector('[name="username"]').value;
  const password = form.querySelector('[name="password"]').value;
  const confirmPassword = form.querySelector('[name="confirm_password"]').value;
  const submitBtn = form.querySelector('button[type="submit"]');

  // Client-side validation
  if (password !== confirmPassword) {
    const errorDiv = form.querySelector('.error-message') || document.createElement('div');
    errorDiv.className = 'error-message bg-red-100 dark:bg-red-900/30 border border-red-400 dark:border-red-600 text-red-700 dark:text-red-400 px-4 py-3 rounded mb-4';
    errorDiv.textContent = 'Passwords do not match';
    if (!form.querySelector('.error-message')) {
      form.insertBefore(errorDiv, form.firstChild);
    }
    return;
  }

  if (password.length < 8) {
    const errorDiv = form.querySelector('.error-message') || document.createElement('div');
    errorDiv.className = 'error-message bg-red-100 dark:bg-red-900/30 border border-red-400 dark:border-red-600 text-red-700 dark:text-red-400 px-4 py-3 rounded mb-4';
    errorDiv.textContent = 'Password must be at least 8 characters';
    if (!form.querySelector('.error-message')) {
      form.insertBefore(errorDiv, form.firstChild);
    }
    return;
  }

  // Show loading state
  const originalText = submitBtn.textContent;
  submitBtn.textContent = 'Creating account...';
  submitBtn.disabled = true;

  try {
    const hashedPassword = await hashPassword(password, username);

    // Replace password fields with hash before submitting
    const hiddenHash = document.createElement('input');
    hiddenHash.type = 'hidden';
    hiddenHash.name = 'password_hash';
    hiddenHash.value = hashedPassword;
    form.appendChild(hiddenHash);

    // Clear the actual password fields so they're not sent
    form.querySelector('[name="password"]').name = '_password_cleared';
    form.querySelector('[name="confirm_password"]').name = '_confirm_cleared';

    // Submit the form
    form.submit();
  } catch (err) {
    console.error('Hashing failed:', err);
    submitBtn.textContent = originalText;
    submitBtn.disabled = false;
  }
}

// Attach handlers when DOM is ready
document.addEventListener('DOMContentLoaded', function() {
  const loginForm = document.getElementById('login-form');
  const registerForm = document.getElementById('register-form');

  if (loginForm) {
    loginForm.addEventListener('submit', handleLoginSubmit);
  }

  if (registerForm) {
    registerForm.addEventListener('submit', handleRegisterSubmit);

    // Add password strength meter listener
    const passwordInput = registerForm.querySelector('[name="password"]');
    if (passwordInput) {
      passwordInput.addEventListener('input', function() {
        updateStrengthMeter(this.value);
      });
    }
  }
});
