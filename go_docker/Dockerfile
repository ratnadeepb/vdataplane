FROM golang:1.16.0-alpine AS builder

WORKDIR /go/src/app
COPY . /go/src/app

RUN CGO_ENABLED=0 go build -o /bin/go-client .

FROM gcr.io/distroless/base
EXPOSE 9999
COPY --from=builder /bin/go-client /go-client
ENTRYPOINT ["/go-client"]