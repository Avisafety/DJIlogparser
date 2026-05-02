FROM node:20-slim AS build
WORKDIR /app
COPY package.json tsconfig.json ./
RUN npm install --omit=optional
COPY src ./src
RUN npx tsc

FROM node:20-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production
COPY package.json ./
RUN npm install --omit=dev --omit=optional && npm cache clean --force
COPY --from=build /app/dist ./dist

EXPOSE 8080
CMD ["node", "dist/server.js"]
