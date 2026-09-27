# Contributing to OMNIX

OMNIX is proprietary, closed-source software. This repository is private and contributions are by invitation only. This guide is for invited collaborators.

## 🌟 Ways to Contribute

- **Bug Reports**: Submit detailed bug reports with reproduction steps
- **Feature Requests**: Propose new features or enhancements
- **Code Contributions**: Submit pull requests for bug fixes or features
- **Documentation**: Improve or translate documentation
- **Testing**: Help test new features and report issues

## 🚀 Getting Started

### 1. Clone (invited collaborators)

```bash
# Requires access to the private repository
git clone https://github.com/paulmmoore3416/omnix-ai-desktop.git
cd omnix-ai-desktop
```

### 2. Set Up Development Environment

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev
```

### 3. Create a Branch

```bash
# Create a feature branch
git checkout -b feature/your-feature-name

# Or a bugfix branch
git checkout -b fix/bug-description
```

## 📝 Code Guidelines

### TypeScript/Svelte

- Use TypeScript for type safety
- Follow Svelte 5 runes syntax (`$state`, `$derived`, etc.)
- Use meaningful variable and function names
- Add comments for complex logic
- Keep components small and focused

### Rust

- Follow Rust naming conventions
- Use `cargo fmt` before committing
- Run `cargo clippy` to catch common issues
- Add error handling with proper error types
- Document public APIs

### Styling

- Use Tailwind CSS utility classes
- Follow the cosmic theme color palette
- Maintain glassmorphic design consistency
- Ensure responsive design

## 🧪 Testing

All of these must pass before a commit (CI runs the same gate):

```bash
# Rust: formatting, lints, tests
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all && cd ..

# Frontend: types, tests, build
npm run check && npm test && npm run build

# kb-core (memory service)
(cd kb-core && python3 -m unittest discover -s tests -t .)
```

When dependencies change, also run `cargo audit` and `npm audit`.

## 📤 Submitting Changes

### 1. Commit Your Changes

```bash
# Stage your changes
git add .

# Commit with a descriptive message
git commit -m "feat: add voice recognition feature"
```

### Commit Message Format

Follow conventional commits:

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `style:` Code style changes (formatting, etc.)
- `refactor:` Code refactoring
- `test:` Adding or updating tests
- `chore:` Maintenance tasks

### 2. Push Your Branch

```bash
git push origin feature/your-feature-name
```

### 3. Create a Pull Request

1. Go to the repository on GitHub
2. Click "New Pull Request"
3. Select your branch
4. Fill out the PR template with:
   - Description of changes
   - Related issue numbers
   - Screenshots (if UI changes)
   - Testing performed

## 🔍 Code Review Process

1. **Automated Checks**: CI/CD will run tests and linting
2. **Maintainer Review**: A maintainer will review your code
3. **Feedback**: Address any requested changes
4. **Approval**: Once approved, your PR will be merged

## 🐛 Reporting Bugs

When reporting bugs, please include:

- **Description**: Clear description of the issue
- **Steps to Reproduce**: Detailed steps to reproduce the bug
- **Expected Behavior**: What should happen
- **Actual Behavior**: What actually happens
- **Environment**: OS, version, etc.
- **Screenshots**: If applicable
- **Logs**: Relevant error messages or logs

## 💡 Feature Requests

When requesting features:

- **Use Case**: Explain why this feature is needed
- **Proposed Solution**: Describe how it should work
- **Alternatives**: Any alternative solutions considered
- **Additional Context**: Screenshots, mockups, etc.

## 📚 Documentation

- Update README.md for user-facing changes
- Add inline code comments for complex logic
- Update API documentation for new endpoints
- Include examples for new features

## 🎨 Design Guidelines

### UI/UX Principles

- **Consistency**: Follow existing design patterns
- **Accessibility**: Ensure keyboard navigation and screen reader support
- **Performance**: Optimize for smooth animations and fast load times
- **Responsiveness**: Test on different screen sizes

### Color Palette

```css
/* Cosmic Theme */
--cosmic-dark: #0a0a0f
--cosmic-darker: #050508
--cosmic-blue: #00d4ff
--cosmic-cyan: #00ffff
--cosmic-purple: #8b5cf6
--cosmic-glow: #00d4ff80
```

## 🔐 Security

- **Never commit sensitive data**: API keys, passwords, etc.
- **Use environment variables**: For configuration
- **Report security issues privately**: Email paulmmoore3416@gmail.com
- **Follow secure coding practices**: Input validation, sanitization

## 📞 Getting Help

- **GitHub Discussions**: For questions and discussions
- **GitHub Issues**: For bug reports and feature requests
- **Email**: paulmmoore3416@gmail.com for private inquiries

## 📜 License

OMNIX is proprietary software (see [LICENSE](LICENSE)). Contributions are by invitation only. By contributing, you assign all rights in your contribution to the owner, as set out in section 3 of the license.

## 🙏 Thank You!

Your contributions make OMNIX better for everyone. We appreciate your time and effort!

---

**Happy Coding! 🚀**