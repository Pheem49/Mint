# คู่มือการ Build & Release (สำหรับ Linux)

ไฟล์นี้จะอธิบายขั้นตอนการสร้างตัวติดตั้ง (Build) ของ Mint สำหรับ Linux และวิธีการปล่อยเวอร์ชันใหม่ (Release) บน GitHub

## 1) การ Build แอป (Linux)

ใช้คำสั่งเหล่านี้เพื่อสร้างไฟล์ติดตั้ง:

```bash
npm install
npm run build:linux
```

เมื่อรันเสร็จ ไฟล์ตัวติดตั้งจะปรากฏในโฟลเดอร์ `dist/` เช่น:
- `Mint-X.Y.Z.tar.gz`
- `mint_X.Y.Z_amd64.deb`

---

## 2) การ Push โค้ดขึ้น GitHub

อัปเดตโค้ดล่าสุดขึ้น Server:ย

```bash
git add .
git commit -m "ใส่ข้อความอธิบายการแก้ไข"
git push
```

**หากเป็นการตั้งค่าครั้งแรก:**
```bash
git init
git remote add origin https://github.com/Pheem49/Luna-Mint.git
git branch -M main
git add .
git commit -m "Initial commit"
git push -u origin main
```

---

## 3) การสร้าง Release บน GitHub ด้วย `gh` CLI

ต้องล็อกอินก่อน (ทำแค่ครั้งแรกครั้งเดียว):
```bash
gh auth login
```

**สร้าง Release ใหม่พร้อมอัปโหลดไฟล์ตัวติดตั้ง:**
(เปลี่ยน `v1.x.x` เป็นเวอร์ชันที่คุณต้องการ เช่น `v1.1.0`)

```bash
# แบบระบุข้อความอธิบายเอง
gh release create v1.5.4 dist/*.deb dist/*.tar.gz --title "Mint v1.5.4" --notes-file RELEASE_NOTES.md


# หรือแบบให้ GitHub สรุปสิ่งที่แก้ไขให้โดยอัตโนมัติ (แนะนำ)
gh release create v1.4.1 dist/*.deb dist/*.tar.gz --generate-notes
```

**หากต้องการอัปโหลดไฟล์เพิ่มเข้าไปใน Release เดิม:**
```bash
gh release upload v1.2.2 dist/*.deb dist/*.tar.gz --clobber
```

---

## 4) วิธีแก้ปัญหา "Source code ในหน้า Release ไม่ใช่ตัวล่าสุด"

ปกติไฟล์ Source code (.zip) จะถูกสร้างจาก **Tag** หากคุณ push โค้ดใหม่แต่ลืมย้าย Tag ให้รันคำสั่งนี้:

```bash
git tag -f v1.5.4
git push -f origin v1.5.4
```
